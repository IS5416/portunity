//! Portunity TUI — Terminal port management dashboard.
//!
//! Tab-based Widget Dashboard (TEA architecture):
//!   [1] Overview  [2] Ports  [3] History  [4] Traffic  [5] Firewall
//!
//! Plan 01-03: interactive fuzzy search ('/'), multi-dimension filter panel ('f'),
//! and admin elevation ('a') with context-sensitive status bar and footer.

mod app;
mod components;
mod elevate;
mod message;
mod theme;
mod update;

use std::io::{self, stdout};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use app::{App, KillTone, LiveMode, WhitelistFocus};
use components::{
    Component, DetailPanelComponent, FilterPanelComponent, FirewallTabComponent,
    HelpComponent, HistoryTabComponent, KillConfirmComponent, OverviewComponent,
    PortsComponent, SearchComponent, TrafficTabComponent, WhitelistOverlayComponent,
};
use message::Message;
use theme::Theme;
use update::update;

use port_core::process::Protection;
use port_core::scanner::PortScanner;

// Phase 3 (CORE-03/SCAN-05): EventBus + monitor wiring for live refresh.
use port_core::events::{CoreEvent, EventBus};
use port_core::monitor::spawn_poller;

// (The former 5s AUTO_REFRESH_INTERVAL is removed — the Phase 3 poller
// (monitor::DEFAULT_POLL_INTERVAL, 2s) drives live refresh via PollTick.)

/// CLI arguments for the TUI binary.
#[derive(Parser)]
#[command(name = "port-tui", about = "Portunity terminal port manager")]
struct Args {
    /// Helper mode: deliver Ctrl+C to the given console process (internal).
    /// Spawned by the port-core kill pipeline with CREATE_NO_WINDOW.
    #[arg(long = "ctrl-c", hide = true)]
    ctrl_c_pid: Option<u32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Helper mode guard (Pitfall 7): --ctrl-c <pid> must exit BEFORE terminal
    // init / raw mode / event loop — the helper only signals a console.
    if let Some(pid) = args.ctrl_c_pid {
        let code = helper_send_ctrl_c(pid);
        std::process::exit(code);
    }

    // Init tracing for stderr (keeps TUI output clean)
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .init();

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channel for async scan results (D-12)
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let mut app = App::new();
    let theme = theme::default_theme();

    // Phase 3 (SCAN-05): EventBus + 2s poller drive live refresh. The TUI
    // subscribes; bus events (PollTick, future NetworkChanged/LiveMode) are
    // converted to mpsc Messages in the event loop below. When the ETW
    // change-trigger lands, it becomes the primary driver and the poller
    // degrades to the fallback cadence (UDP/edge cases).
    let bus = EventBus::new();
    spawn_poller(bus.clone(), port_core::monitor::DEFAULT_POLL_INTERVAL);
    let mut bus_rx = bus.subscribe();

    // Admin check at startup (D-09: run once, result persists for session)
    let is_admin = elevate::is_admin();
    let _ = tx.send(Message::AdminCheck(is_admin));

    // Spawn initial scan (D-15: first frame shows scanning indicator)
    spawn_scan(tx.clone());

    // Track whether a scan has been spawned for the current request
    let mut scan_spawned = true; // initial scan already spawned

    // Main event loop
    let result = run_event_loop(
        &mut terminal,
        &mut app,
        &theme,
        &tx,
        &mut rx,
        &mut bus_rx,
        &mut scan_spawned,
    );

    // Cleanup — always executed regardless of how the loop exits
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    result
}

/// Spawn an async scan task, sending results via the channel.
fn spawn_scan(tx: tokio::sync::mpsc::UnboundedSender<Message>) {
    tokio::spawn(async move {
        let scanner = port_core::windows::WindowsPortScanner;
        match scanner.scan().await {
            Ok(conns) => {
                let _ = tx.send(Message::ScanComplete(conns));
            }
            Err(e) => {
                let _ = tx.send(Message::ScanError(e.to_string()));
            }
        }
    });
}

/// Spawn the on-demand detail fetch (D-08) — `fetch_details` runs its own
/// spawn_blocking scope in port-core; the result flows back via
/// `DetailDataLoaded` through the mpsc channel. The row's scan-time
/// ProcessInfo is the fallback: on fetch failure the panel keeps name/PID
/// and the protection markers with every other field at "—" (UI-SPEC
/// Detail Panel error state).
fn spawn_detail_fetch(tx: tokio::sync::mpsc::UnboundedSender<Message>, row: port_core::models::ProcessInfo) {
    let pid = row.pid;
    let fallback_name = row.name.clone();
    tokio::spawn(async move {
        let process_info = match port_core::process::fetch_details(pid).await {
            Ok(mut info) => {
                if info.name.is_empty() {
                    info.name = fallback_name;
                }
                info
            }
            Err(_) => port_core::models::ProcessInfo {
                executable_path: None,
                command_line: None,
                start_time: None,
                is_signed: None,
                ..row
            },
        };
        let _ = tx.send(Message::DetailDataLoaded { process_info });
    });
}

/// Deliver Ctrl+C to a console process (helper mode, `--ctrl-c <pid>`).
///
/// Exit-code contract (documented in `port-core/src/process/kill.rs`):
/// `0` = delivered, `1` = no console, `2` = delivery failed. The helper
/// ignores CTRL_C in itself,
/// detaches from its own hidden console, attaches to the target's console,
/// and broadcasts CTRL_C_EVENT to all processes on that console (group 0).
/// The helper cannot terminate anything — it only generates a console event.
///
/// Caveats: the event broadcasts to ALL processes on the target's console;
/// CREATE_NEW_PROCESS_GROUP children ignore Ctrl+C; a pending ReadConsole may
/// not be interrupted — delivery ≠ exit, the kill pipeline's WaitForSingleObject
/// timeout is the real arbiter.
fn helper_send_ctrl_c(pid: u32) -> i32 {
    use windows::Win32::System::Console::{
        AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
        CTRL_C_EVENT,
    };

    unsafe {
        // Ignore CTRL_C in the helper so the broadcast does not kill it.
        let _ = SetConsoleCtrlHandler(None, true);
        // Detach the helper's own (hidden) console — never the TUI's terminal.
        let _ = FreeConsole();

        match AttachConsole(pid) {
            Ok(()) => {
                // WR-05: report the event result truthfully — a failed
                // GenerateConsoleCtrlEvent must NOT claim "delivered"
                // (the pipeline would wait the full graceful timeout for
                // a signal that was never sent).
                if GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_ok() {
                    0 // delivered
                } else {
                    2 // event delivery failed
                }
            }
            Err(_) => 1, // no console
        }
    }
}

/// Run the TEA event loop with auto-refresh and keyboard navigation.
///
/// Renders on-demand: only redraws when state changes or the per-second
/// clock refresh fires. This eliminates the constant 5 fps idle redraw.
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    theme: &Theme,
    tx: &tokio::sync::mpsc::UnboundedSender<Message>,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Message>,
    bus_rx: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
    scan_spawned: &mut bool,
) -> Result<()> {
    // Minimum interval between forced renders (clock update, 1 Hz).
    const CLOCK_TICK: Duration = Duration::from_secs(1);

    loop {
        // Track terminal width — truncation budget for status/footer strings
        if let Ok(size) = terminal.size() {
            app.term_width = size.width;
        }

        // Render only when needed, but at least once per second for clock
        let now = std::time::Instant::now();
        let clock_due = app
            .last_render
            .map_or(true, |last| now.duration_since(last) >= CLOCK_TICK);

        if app.needs_render || clock_due {
            terminal.draw(|f| render_app(f, app, theme))?;
            app.needs_render = false;
            app.last_render = Some(now);
        }

        if app.should_quit {
            break;
        }

        // Phase 3 (SCAN-05): drain bus events. The poller publishes PollTick
        // every 2s, which triggers a rescan here (respecting the in-flight
        // guard — same posture as the 5s auto-refresh it replaces). Future
        // NetworkChanged (ETW trigger) and LiveMode events land in this match
        // too, so the TUI never needs to know how the trigger is produced.
        while let Ok(evt) = bus_rx.try_recv() {
            match evt {
                CoreEvent::PollTick => {
                    if !app.scanning && app.error.is_none() {
                        app.scanning = true;
                        app.needs_render = true;
                        spawn_scan(tx.clone());
                        *scan_spawned = true;
                    }
                }
                CoreEvent::LiveMode { etw } => {
                    app.live_mode = if etw { LiveMode::Etw } else { LiveMode::Polling };
                    app.needs_render = true;
                }
                _ => {} // PortsScanned/NetworkChanged/TrafficUpdate consumed elsewhere
            }
        }

        // Poll timeout: short when scanning (need to catch completion),
        // longer when idle (no work to do).
        let poll_dur = if app.scanning {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(500)
        };

        if event::poll(poll_dur)? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events — skip Release/Repeat
                // to prevent double-firing of all keyboard actions.
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                let msg = map_key_event(key, app);
                if let Some(m) = msg {
                    match m {
                        // Intercept ElevateRequest: spawn blocking elevation task
                        Message::ElevateRequest => {
                            if !app.elevating {
                                app.elevating = true;
                                let tx_elevate = tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    match elevate::elevate_to_admin() {
                                        Ok(()) => {
                                            let _ =
                                                tx_elevate.send(Message::ElevateDeclined);
                                        }
                                        Err(e) => {
                                            let _ = tx_elevate.send(Message::ElevateFailed(
                                                format!("Elevation failed: {}", e),
                                            ));
                                        }
                                    }
                                });
                            }
                        }
                        // Intercept Kill: snapshot + protection verdict off the
                        // async runtime (two-stage gate, D-09/D-15)
                        Message::Kill { pid } => {
                            if !app.kill_in_flight && app.confirm_pid.is_none() {
                                let name = app
                                    .display_data()
                                    .get(app.selected_index)
                                    .map(|c| c.process.name.clone())
                                    .unwrap_or_default();
                                let tx_kill = tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    match port_core::process::snapshot_for(pid) {
                                        Err(e) => {
                                            // Process gone or inaccessible —
                                            // map to a truthful outcome.
                                            let outcome = match e {
                                                port_core::Error::PermissionDenied(
                                                    _,
                                                ) => {
                                                    port_core::process::KillOutcome::AccessDenied
                                                }
                                                _ => port_core::process::KillOutcome::AlreadyExited,
                                            };
                                            let _ = tx_kill.send(Message::KillOutcome {
                                                outcome,
                                                name,
                                                pid,
                                            });
                                        }
                                        Ok(snap) => {
                                            // D-15: settings re-read at kill time
                                            let settings = port_core::config::load_settings()
                                                .unwrap_or_else(|_| {
                                                    port_core::config::default_settings()
                                                });
                                            let basename = snap
                                                .executable_path
                                                .as_deref()
                                                .and_then(|p| {
                                                    std::path::Path::new(p)
                                                        .file_name()
                                                        .and_then(|f| f.to_str())
                                                })
                                                .unwrap_or(&name);
                                            let protection =
                                                port_core::process::protection_status(
                                                    snap.pid,
                                                    basename,
                                                    snap.executable_path.as_deref(),
                                                    &settings,
                                                );
                                            let _ = tx_kill.send(Message::KillPrepared {
                                                snapshot: snap,
                                                protection,
                                                name,
                                                pid,
                                            });
                                        }
                                    }
                                });
                            }
                        }
                        // Intercept KillConfirmed: execute the pending kill with
                        // the verified snapshot (PROC-07)
                        Message::KillConfirmed { pid: confirm_pid } => {
                            let snapshot_ok = app
                                .pending_kill_snapshot
                                .as_ref()
                                .is_some_and(|s| s.pid == confirm_pid);
                            if snapshot_ok && !app.kill_in_flight {
                                let snapshot = app.pending_kill_snapshot.take().unwrap();
                                app.kill_in_flight = true;
                                let name = app
                                    .confirm_name
                                    .clone()
                                    .unwrap_or_else(|| "process".to_string());
                                let pid = snapshot.pid;
                                app.kill_timeout_secs = port_core::config::load_settings()
                                    .map(|s| s.kill_timeout_secs)
                                    .unwrap_or(5);
                                let timeout = app.kill_timeout_secs;
                                let tx_kill = tx.clone();
                                let tx_timeout = tx_kill.clone();
                                let name_timeout = name.clone();
                                tokio::spawn(async move {
                                    let _ = tx_kill.send(Message::KillStart {
                                        name: name.clone(),
                                        pid,
                                    });
                                    let outcome = port_core::process::kill(
                                        snapshot,
                                        timeout,
                                        move || {
                                            let _ = tx_timeout.send(Message::KillTimeout {
                                                name: name_timeout.clone(),
                                                pid,
                                                timeout_secs: timeout,
                                            });
                                        },
                                    )
                                    .await;
                                    let _ = tx_kill.send(Message::KillOutcome {
                                        outcome,
                                        name,
                                        pid,
                                    });
                                });
                            }
                        }
                        // Intercept WhitelistAdd: validate the path off the
                        // async runtime, then persist via save_settings (D-15
                        // — the overlay's working copy + the new entry). The
                        // validation + save run inside spawn_blocking (file
                        // I/O + Win32 GetLongPathNameW off the runtime).
                        // Duplicate add (case-insensitive) is a no-op — no
                        // save, WhitelistSaved { added: false } shows the
                        // "already on your protection list" info string.
                        Message::WhitelistAdd { path } => {
                            if !app.kill_in_flight {
                                let tx_wl = tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    match port_core::process::validate_user_entry(&path) {
                                        Ok(normalized) => {
                                            // WR-04: re-read settings FRESH in the
                                            // blocking closure and merge onto that
                                            // copy — never save a stale working
                                            // copy. Concurrent add/delete saves can
                                            // no longer clobber each other (last
                                            // writer always started from the latest
                                            // on-disk state).
                                            let mut settings =
                                                match port_core::config::load_settings()
                                                {
                                                    Ok(s) => s,
                                                    Err(e) => {
                                                        let _ = tx_wl.send(
                                                            Message::WhitelistError {
                                                                path,
                                                                reason: format!(
                                                                    "could not load settings: {}",
                                                                    e
                                                                ),
                                                            },
                                                        );
                                                        return;
                                                    }
                                                };
                                            let is_dup = settings
                                                .whitelist
                                                .iter()
                                                .any(|e| {
                                                    e.eq_ignore_ascii_case(&normalized)
                                                });
                                            if is_dup {
                                                let _ = tx_wl.send(Message::WhitelistSaved {
                                                    path: normalized,
                                                    added: false,
                                                });
                                            } else {
                                                settings.whitelist.push(normalized.clone());
                                                match port_core::config::save_settings(
                                                    &settings,
                                                ) {
                                                    Ok(()) => {
                                                        let _ = tx_wl.send(
                                                            Message::WhitelistSaved {
                                                                path: normalized,
                                                                added: true,
                                                            },
                                                        );
                                                    }
                                                    Err(e) => {
                                                        let _ = tx_wl.send(
                                                            Message::WhitelistError {
                                                                path,
                                                                reason: format!(
                                                                    "could not save settings: {}",
                                                                    e
                                                                ),
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        Err(reason) => {
                                            let _ = tx_wl.send(Message::WhitelistError {
                                                path,
                                                reason,
                                            });
                                        }
                                    }
                                });
                            }
                        }
                        // Intercept WhitelistDeleteSelected: bounds-checked
                        // removal (removal of a non-existent index is a no-op
                        // — guard before spawning). The working copy is
                        // mutated FIRST (main-loop-owned state, PROC-05), then
                        // the post-removal state is persisted off-runtime;
                        // WhitelistSaved { added: false } completes the status
                        // string. No confirmation (D-15 — reversible by
                        // re-adding).
                        Message::WhitelistDeleteSelected => {
                            if !app.kill_in_flight {
                                let idx = app.whitelist_selected;
                                if idx < app.whitelist_settings.whitelist.len() {
                                    let removed =
                                        app.whitelist_settings.whitelist.remove(idx);
                                    if app.whitelist_selected
                                        >= app.whitelist_settings.whitelist.len()
                                    {
                                        app.whitelist_selected = app
                                            .whitelist_settings
                                            .whitelist
                                            .len()
                                            .saturating_sub(1);
                                    }
                                    let tx_wl = tx.clone();
                                    tokio::task::spawn_blocking(move || {
                                        // WR-04: re-read settings FRESH and remove
                                        // by path (case-insensitive) — never save a
                                        // stale working copy. A concurrent add's
                                        // entry survives this save because the fresh
                                        // copy already contains it; an index-based
                                        // removal against a stale clone would drop
                                        // it.
                                        let mut settings =
                                            match port_core::config::load_settings() {
                                                Ok(s) => s,
                                                Err(e) => {
                                                    let _ = tx_wl.send(
                                                        Message::WhitelistError {
                                                            path: removed.clone(),
                                                            reason: format!(
                                                                "could not load settings: {}",
                                                                e
                                                            ),
                                                        },
                                                    );
                                                    return;
                                                }
                                            };
                                        if let Some(pos) = settings
                                            .whitelist
                                            .iter()
                                            .position(|e| {
                                                e.eq_ignore_ascii_case(&removed)
                                            })
                                        {
                                            settings.whitelist.remove(pos);
                                        }
                                        match port_core::config::save_settings(&settings) {
                                            Ok(()) => {
                                                let _ = tx_wl.send(
                                                    Message::WhitelistSaved {
                                                        path: removed,
                                                        added: false,
                                                    },
                                                );
                                            }
                                            Err(e) => {
                                                let _ = tx_wl.send(
                                                    Message::WhitelistError {
                                                        path: removed,
                                                        reason: format!(
                                                            "could not save settings: {}",
                                                            e
                                                        ),
                                                    },
                                                );
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        // Intercept ToggleDetailPanel: on OPEN, capture the
                        // selected row as the panel's PID and fire the on-demand
                        // detail fetch (D-08) off the async runtime. On fetch
                        // failure the panel keeps name/PID from the row with
                        // every other field at "—" (UI-SPEC error state: close
                        // and re-open to retry).
                        Message::ToggleDetailPanel => {
                            let opening = !app.detail_active;
                            if opening && !app.detail_loading {
                                let row = app.selected_connection().map(|c| c.process.clone());
                                match row {
                                    Some(row) => {
                                        app.detail_pid = Some(row.pid);
                                        app.detail_loading = true;
                                        app.detail_exited = false;
                                        app.detail_data = None;
                                        spawn_detail_fetch(tx.clone(), row);
                                    }
                                    None => {
                                        app.detail_pid = None;
                                    }
                                }
                            }
                            update(app, Message::ToggleDetailPanel);
                            app.needs_render = true;
                        }
                        // Intercept selection movement while the panel is open:
                        // selection change refreshes the panel content (D-06).
                        Message::MoveUp | Message::MoveDown | Message::ScrollTop
                        | Message::ScrollBottom => {
                            update(app, m);
                            if app.detail_active && !app.detail_loading {
                                let row = app.selected_connection().map(|c| c.process.clone());
                                if let Some(row) = row {
                                    if app.detail_pid != Some(row.pid) {
                                        app.detail_pid = Some(row.pid);
                                        app.detail_loading = true;
                                        app.detail_exited = false;
                                        app.detail_data = None;
                                        spawn_detail_fetch(tx.clone(), row);
                                    }
                                }
                            }
                            app.needs_render = true;
                        }
                        other => {
                            update(app, other);
                            app.needs_render = true;
                        }
                    }
                }
            }
        }

        // Drain async channel (D-12: try_recv on each tick)
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Message::ScanComplete(conns) => {
                    *scan_spawned = false;
                    // D-07: invalidate the signature cache on every scan —
                    // no stale signature is ever displayed (T-02-07).
                    app.signature_cache.clear();
                    // ProcessExited detection: the shown process left the
                    // scan list — render strikethrough + "Exited".
                    if app.detail_active {
                        if let Some(dpid) = app.detail_pid {
                            if !conns.iter().any(|c| c.process.pid == dpid) {
                                update(app, Message::ProcessExited { pid: dpid });
                            }
                        }
                    }
                    update(app, Message::ScanComplete(conns));
                }
                // DetailDataLoaded drain special-case: store the data, then
                // fire the on-demand signature verification (D-07) when the
                // per-PID cache misses — update() cannot spawn. On cache hit
                // the panel renders the cached verdict immediately.
                Message::DetailDataLoaded { process_info } => {
                    if Some(process_info.pid) == app.detail_pid {
                        let pid = process_info.pid;
                        let path = process_info.executable_path.clone();
                        update(app, Message::DetailDataLoaded { process_info });
                        if let Some(path) = path {
                            if !app.signature_cache.contains_key(&pid) {
                                let tx_sig = tx.clone();
                                tokio::spawn(async move {
                                    let is_signed =
                                        port_core::process::verify_signature(&path).await;
                                    let _ = tx_sig.send(Message::SignatureVerified {
                                        pid,
                                        is_signed,
                                    });
                                });
                            }
                        } else {
                            // No path — no signature possible; render Unknown
                            // (never leave the row stuck on "Verifying…").
                            app.signature_cache.insert(pid, None);
                        }
                    }
                    // Stale result (selection moved on) — dropped.
                }
                Message::ScanError(e) => {
                    // A scan lifecycle ends here too (CR-01): reset the spawn
                    // guard or the next Refresh finds `scan_spawned == true`,
                    // never spawns a new scan, and the TUI stays stuck on
                    // "Scanning..." forever. update() sets scanning=false and
                    // surfaces the error.
                    *scan_spawned = false;
                    update(app, Message::ScanError(e));
                }
                Message::KillPrepared {
                    snapshot,
                    protection,
                    name,
                    pid,
                } => {
                    // Instant-kill path for non-protected processes (PROC-03):
                    // Protection::None never reaches update() — the kill task
                    // is spawned directly with the incoming snapshot (same body
                    // as the KillConfirmed path; guarded by kill_in_flight).
                    if protection == Protection::None {
                        if !app.kill_in_flight {
                            app.kill_in_flight = true;
                            app.kill_timeout_secs = port_core::config::load_settings()
                                .map(|s| s.kill_timeout_secs)
                                .unwrap_or(5);
                            let timeout = app.kill_timeout_secs;
                            let tx_kill = tx.clone();
                            let tx_timeout = tx_kill.clone();
                            let name_timeout = name.clone();
                            tokio::spawn(async move {
                                let _ = tx_kill.send(Message::KillStart {
                                    name: name.clone(),
                                    pid,
                                });
                                let outcome = port_core::process::kill(
                                    snapshot,
                                    timeout,
                                    move || {
                                        let _ = tx_timeout.send(Message::KillTimeout {
                                            name: name_timeout.clone(),
                                            pid,
                                            timeout_secs: timeout,
                                        });
                                    },
                                )
                                .await;
                                let _ = tx_kill.send(Message::KillOutcome {
                                    outcome,
                                    name,
                                    pid,
                                });
                            });
                        }
                    } else {
                        // UserConfirm -> dialog state; HardBlocked -> status
                        update(
                            app,
                            Message::KillPrepared {
                                snapshot,
                                protection,
                                name,
                                pid,
                            },
                        );
                    }
                }
                other => update(app, other),
            }
            app.needs_render = true;
        }

        // Spawn scan if scanning flag is set and we haven't spawned one yet
        if app.scanning && !*scan_spawned {
            spawn_scan(tx.clone());
            *scan_spawned = true;
        }
        // (The former 5s AUTO_REFRESH block is replaced by the Phase 3
        // poller-driven PollTick handler above — Wave 3.1, SCAN-05.)
    }

    Ok(())
}

/// Map a crossterm KeyEvent to an optional Message.
///
/// Handles mode-specific key dispatch: search mode, filter mode, and default mode.
/// All non-overlay keys (r, q, j, k, s, etc.) pass through when search/filter is active.
fn map_key_event(key: crossterm::event::KeyEvent, app: &App) -> Option<Message> {
    // --- Confirm dialog dispatch (TOPMOST overlay — UI-SPEC L2-confirm) ---
    // Hoisted ABOVE the search/filter blocks: the dialog renders on top of
    // every other overlay, so its y/Enter/n/Esc/x keys must win regardless of
    // whether the search bar or filter panel is also active. Previously the
    // search (and filter) blocks ran first and swallowed the confirm keys,
    // producing a stuck modal: open '/', move to a protected row, press 'x' —
    // y/n/Enter became no-ops (TUI review W-1). Other keys still fall through
    // to their normal dispatch (UI-SPEC confirm pass-through).
    if app.confirm_pid.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                return Some(Message::KillConfirmed {
                    pid: app.confirm_pid.unwrap_or(0),
                });
            }
            KeyCode::Char('n') | KeyCode::Esc => return Some(Message::KillCancelled),
            KeyCode::Char('x') => return None, // no-op: prevents re-triggering kill (UI-SPEC)
            // Pass-through: all other keys keep working (UI-SPEC confirm table)
            _ => {}
        }
    }

    // --- Search mode dispatch ---
    if app.search_active {
        match key.code {
            KeyCode::Esc => return Some(Message::SearchDeactivate),
            KeyCode::Enter => return Some(Message::SearchDeactivate),
            KeyCode::Backspace => return Some(Message::SearchBackspace),
            KeyCode::Left => return Some(Message::SearchCursorLeft),
            KeyCode::Right => return Some(Message::SearchCursorRight),
            KeyCode::Char(ch) => {
                // All printable chars go to search input
                if !ch.is_control() {
                    return Some(Message::SearchInput(ch));
                }
                // Pass through control chars for universal commands
            }
            // Pass-through: all other keys (j, k, r, s, etc.) continue to work
            _ => {}
        }
    }

    // --- Filter mode dispatch ---
    if app.filter_active {
        match key.code {
            KeyCode::Esc => return Some(Message::FilterDeactivate),
            KeyCode::Enter => return Some(Message::FilterApply),
            KeyCode::Tab => return Some(Message::FilterTabField),
            KeyCode::BackTab => {
                // Shift+Tab: cycle backward
                return Some(Message::FilterTabBackward);
            }
            KeyCode::Char(ch) => {
                if !ch.is_control() {
                    return Some(Message::FilterUpdateField(
                        app.filter_focused_field.clone(),
                        ch.to_string(),
                    ));
                }
            }
            KeyCode::Backspace => return Some(Message::FilterFieldBackspace),
            // Pass-through: j, k, r, s continue to work
            _ => {}
        }
    }

    // --- Detail panel dispatch (D-06) — d/Esc close only; everything else
    // (j/k/up/down/r/s/g/G//f/x) passes through to the table (UI-SPEC
    // detail pass-through table). Placed AFTER the confirm dialog so the
    // topmost overlay keeps Esc semantics. ---
    if app.detail_active {
        match key.code {
            KeyCode::Char('d') | KeyCode::Esc => return Some(Message::ToggleDetailPanel),
            _ => {}
        }
    }

    // --- Help overlay dispatch ('?') — universal full-area reference
    // overlay. Esc and '?' close it; everything else is swallowed (the
    // overlay covers the whole content area — no hidden table interaction,
    // and the confirm dialog keeps its topmost keys since its dispatch runs
    // earlier). ---
    if app.help_active {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => return Some(Message::ToggleHelp),
            _ => return None,
        }
    }

    // --- Whitelist overlay dispatch (D-14) — UI-SPEC whitelist pass-through
    // table: j/k/↑/↓ (user-list focus), d (delete selected entry),
    // Tab/Shift+Tab (focus user-list ↔ input), printable chars + Backspace +
    // ←/→ (input focus), Enter (add path), Esc (close). r/s and L0 tab/quit
    // keys pass through to the default match below (the declared exception:
    // j/k do NOT pass through to the table — the overlay covers it). ---
    if app.whitelist_active {
        match key.code {
            KeyCode::Esc => return Some(Message::ToggleWhitelistOverlay),
            KeyCode::Tab => return Some(Message::WhitelistFocusNext),
            KeyCode::BackTab => return Some(Message::WhitelistFocusPrev),
            KeyCode::Char('j') | KeyCode::Down if app.whitelist_focus == WhitelistFocus::List => {
                return Some(Message::WhitelistSelectMove { dir: 1 });
            }
            KeyCode::Char('k') | KeyCode::Up if app.whitelist_focus == WhitelistFocus::List => {
                return Some(Message::WhitelistSelectMove { dir: -1 });
            }
            KeyCode::Char('d') if app.whitelist_focus == WhitelistFocus::List => {
                return Some(Message::WhitelistDeleteSelected);
            }
            KeyCode::Enter if app.whitelist_focus == WhitelistFocus::Input => {
                return Some(Message::WhitelistAdd {
                    path: app.whitelist_input.clone(),
                });
            }
            KeyCode::Backspace if app.whitelist_focus == WhitelistFocus::Input => {
                return Some(Message::WhitelistBackspace);
            }
            KeyCode::Left if app.whitelist_focus == WhitelistFocus::Input => {
                return Some(Message::WhitelistCursorMove { dir: -1 });
            }
            KeyCode::Right if app.whitelist_focus == WhitelistFocus::Input => {
                return Some(Message::WhitelistCursorMove { dir: 1 });
            }
            KeyCode::Char(ch) if app.whitelist_focus == WhitelistFocus::Input && !ch.is_control() => {
                return Some(Message::WhitelistInput(ch));
            }
            // Everything else falls through to the tab-switch / default
            // matches below (r/s and tab/quit keys pass per UI-SPEC).
            _ => {}
        }
    }

    // --- Tab switching (works in all modes) ---
    match key.code {
        KeyCode::Char('1') => return Some(Message::SwitchTab(0)),
        KeyCode::Char('2') => return Some(Message::SwitchTab(1)),
        KeyCode::Char('3') => return Some(Message::SwitchTab(2)),
        KeyCode::Char('4') => return Some(Message::SwitchTab(3)),
        KeyCode::Char('5') => return Some(Message::SwitchTab(4)),
        KeyCode::Tab => return Some(Message::SwitchTab((app.active_tab + 1) % 5)),
        KeyCode::BackTab => return Some(Message::SwitchTab((app.active_tab + 4) % 5)),
        _ => {}
    }

    // --- Default mode dispatch (when no overlay is active) ---
    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('w') => {
            // Whitelist overlay toggle (D-14) — available from any tab
            // (UI-SPEC); pressing w again closes it (Esc-equivalent).
            Some(Message::ToggleWhitelistOverlay)
        }
        KeyCode::Char('?') => {
            // Help overlay — universal (UI-SPEC: the canonical reference for
            // the footer-dropped s/w keys). Works on any tab.
            Some(Message::ToggleHelp)
        }
        KeyCode::Char('r') => Some(Message::Refresh),
        KeyCode::Char('s') => Some(Message::Sort(app.sort_column)),
        KeyCode::Char('d') => {
            // Detail panel toggle (D-06) — Ports tab only (UI-SPEC).
            // When the panel is open 'd' is already intercepted above.
            if app.active_tab == 1 {
                Some(Message::ToggleDetailPanel)
            } else {
                None
            }
        }
        KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Char('g') => Some(Message::ScrollTop),
        KeyCode::Char('G') => Some(Message::ScrollBottom),
        KeyCode::Char('/') => {
            if !app.search_active && !app.filter_active {
                Some(Message::SearchActivate)
            } else {
                None
            }
        }
        KeyCode::Char('f') => {
            if !app.search_active && !app.filter_active {
                Some(Message::FilterActivate)
            } else {
                None
            }
        }
        KeyCode::Char('x') => {
            // Kill owning process (D-01). No-op on empty list (PROC-01 edge
            // truth: no selected row -> no kill, no dialog).
            if app.confirm_pid.is_none() {
                if let Some(conn) = app.display_data().get(app.selected_index) {
                    Some(Message::Kill {
                        pid: conn.process.pid,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        KeyCode::Char('a') => {
            // Elevation: only when not already admin, not in overlay, and check done
            if !app.is_admin && app.admin_check_done && !app.search_active && !app.filter_active {
                Some(Message::ElevateRequest)
            } else {
                None
            }
        }
        KeyCode::Esc => {
            if app.filter_applied {
                Some(Message::FilterDeactivate)
            } else if app.error.is_some() {
                None
            } else {
                Some(Message::Quit)
            }
        }
        _ => None,
    }
}

/// Render the full application frame.
///
/// Enforces the resize gate (TUI-07): if terminal < 80x24, renders a centered
/// "Terminal too small" message and returns without rendering the normal layout.
/// Otherwise renders the full 4-region layout: tab bar, content, status bar, footer.
fn render_app(f: &mut Frame, app: &App, theme: &Theme) {
    let area = f.area();

    // Resize gate (TUI-07): enforce minimum 80x24 terminal size
    if area.width < 80 || area.height < 24 {
        let text = Text::from(vec![
            Line::from(Span::styled(
                "Terminal too small",
                Style::default()
                    .fg(theme.fg_muted)
                    .bg(theme.bg_base)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "Minimum size: 80 columns x 24 rows. Current: {}x{}",
                    area.width, area.height
                ),
                Style::default().fg(theme.fg_muted).bg(theme.bg_base),
            )),
            Line::from(Span::styled(
                "Resize your terminal window to continue.",
                Style::default().fg(theme.fg_muted).bg(theme.bg_base),
            )),
        ]);
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().bg(theme.bg_base));
        f.render_widget(paragraph, area);
        return;
    }

    // Layout: tab_bar (1), content (fill), status_bar (1), footer (1)
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let tab_bar_area = layout[0];
    let content_area = layout[1];
    let status_bar_area = layout[2];
    let footer_area = layout[3];

    // Tab bar with active tab highlighting
    render_tab_bar(f, tab_bar_area, app, theme);

    // Content area dispatch: render active tab component
    // Search and filter overlays only apply on Ports tab (tab 1)
    if app.active_tab == 1 {
        // Adjust content area for overlays: search (3 rows), filter (5 rows) stack at top
        let overlay_offset = if app.search_active { 3u16 } else { 0u16 }
            + if app.filter_active { 7u16 } else { 0u16 };

        let table_area = if overlay_offset > 0 && content_area.height > overlay_offset {
            Rect {
                y: content_area.y + overlay_offset,
                height: content_area.height.saturating_sub(overlay_offset),
                ..content_area
            }
        } else {
            content_area
        };

        // Port table (below overlays)
        PortsComponent.render(app, f, table_area, theme);

        // Overlays: search bar on top, filter panel below it
        if app.search_active {
            let search_overlay = Rect {
                height: 3,
                ..content_area
            };
            SearchComponent.render(app, f, search_overlay, theme);
        }

        if app.filter_active {
            let filter_y = if app.search_active {
                content_area.y + 3
            } else {
                content_area.y
            };
            let filter_overlay = Rect {
                y: filter_y,
                height: 7,
                ..content_area
            };
            FilterPanelComponent.render(app, f, filter_overlay, theme);
        }

        // Detail panel — 12-row top-anchored Clear-over (D-05: the table is
        // never squeezed; >=9 table rows stay visible at 80x24). Stack order
        // per UI-SPEC: table -> search -> filter -> detail -> (whitelist,
        // plan 02-03) -> (help) -> confirm.
        if app.detail_active {
            let detail_area = Rect {
                height: 12,
                ..content_area
            };
            DetailPanelComponent.render(app, f, detail_area, theme);
        }
    } else {
        // Other tabs get full content area
        match app.active_tab {
            0 => OverviewComponent.render(app, f, content_area, theme),
            2 => HistoryTabComponent.render(app, f, content_area, theme),
            3 => TrafficTabComponent.render(app, f, content_area, theme),
            4 => FirewallTabComponent.render(app, f, content_area, theme),
            _ => {} // unreachable — guarded by update bounds check
        }
    }

    // Whitelist overlay (D-14) — any tab (UI-SPEC: w available from any tab).
    // Full content width x (content height - 1) rows, Clear-over (20 rows at
    // 80x24). Renders above the tab content (incl. the detail panel) and
    // below help and the confirm dialog (UI-SPEC overlay stack).
    if app.whitelist_active {
        let wl_area = Rect {
            height: content_area.height.saturating_sub(1),
            ..content_area
        };
        WhitelistOverlayComponent.render(app, f, wl_area, theme);
    }

    // Help overlay ('?') — full content area, renders above the whitelist
    // overlay and below the confirm dialog (UI-SPEC stack order; the
    // canonical reference for the footer-dropped s/w keys).
    if app.help_active {
        HelpComponent.render(app, f, content_area, theme);
    }

    // Kill confirmation dialog — centered 60x7 popup, always topmost
    // (UI-SPEC overlay stack: table -> search -> filter -> detail ->
    // whitelist -> help -> confirm). Only reachable from the Ports tab.
    if app.confirm_pid.is_some() {
        let w = content_area.width;
        let h = content_area.height;
        let confirm_area = Rect {
            x: content_area.x + (w.saturating_sub(60)) / 2,
            y: content_area.y + (h.saturating_sub(7)) / 2,
            width: 60.min(w),
            height: 7.min(h),
        };
        KillConfirmComponent.render(app, f, confirm_area, theme);
    }

    // Status bar
    render_status_bar(f, status_bar_area, app, theme);

    // Footer
    render_footer(f, footer_area, app, theme);
}

/// Render the tab bar with active tab highlighted (Bold + accent_primary bg).
///
/// Active tab: Bold + fg in bg_base + bg in accent_primary (reverse contrast).
/// Inactive tabs: Dim + fg_muted + bg_surface.
fn render_tab_bar(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let tab_labels = [
        " [1] Overview ",
        " [2] Ports ",
        " [3] History ",
        " [4] Traffic ",
        " [5] Firewall ",
    ];

    let active_style = Style::default()
        .fg(theme.bg_base)
        .bg(theme.accent_primary)
        .add_modifier(Modifier::BOLD);

    let inactive_style = Style::default()
        .fg(theme.fg_muted)
        .bg(theme.bg_surface)
        .add_modifier(Modifier::DIM);

    let sep_style = Style::default()
        .fg(theme.fg_muted)
        .bg(theme.bg_surface);

    let mut spans: Vec<Span> = Vec::new();

    for (i, label) in tab_labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ", sep_style));
        }
        if i == app.active_tab {
            spans.push(Span::styled(*label, active_style));
        } else {
            spans.push(Span::styled(*label, inactive_style));
        }
    }

    let tabs = Paragraph::new(Text::from(Line::from(spans)))
        .style(Style::default().bg(theme.bg_surface));
    f.render_widget(tabs, area);
}

/// Render the status bar with context-sensitive message.
///
/// Includes admin status indicator per UI-SPEC: "Admin \u{2713}" in green for admin,
/// "Admin needed \u{2014} press a to elevate" in yellow for non-admin.
fn render_status_bar(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    // Build admin status suffix
    let admin_suffix = if app.admin_check_done {
        if app.is_admin {
            Span::styled(
                " \u{00b7} Admin \u{2713}",
                Style::default().fg(theme.status_success),
            )
        } else {
            Span::styled(
                " \u{00b7} Admin needed \u{2014} press a to elevate",
                Style::default().fg(theme.status_warning),
            )
        }
    } else {
        // Admin check not done yet — don't show admin status to prevent flicker
        Span::raw("")
    };

    let base_style = Style::default().bg(theme.bg_surface);

    if app.scanning {
        let spans = vec![
            Span::styled("Scanning...", base_style.fg(theme.fg_default)),
            Span::styled(
                format!(" \u{00b7} {} found so far", app.ports.len()),
                base_style.fg(theme.fg_muted),
            ),
            Span::styled(
                format!(" \u{00b7} {}", chrono::Local::now().format("%H:%M:%S")),
                base_style.fg(theme.fg_muted),
            ),
            admin_suffix,
        ];
        let paragraph = Paragraph::new(Text::from(Line::from(spans))).style(base_style);
        f.render_widget(paragraph, area);
    } else if let Some(ref e) = app.error {
        let spans = vec![
            Span::styled(
                format!("\u{26a0} {}", e),
                Style::default().fg(theme.status_error).bg(theme.bg_surface),
            ),
            Span::styled(
                " \u{00b7} Press r to retry",
                Style::default().fg(theme.fg_muted).bg(theme.bg_surface),
            ),
        ];
        let paragraph = Paragraph::new(Text::from(Line::from(spans)))
            .style(Style::default().bg(theme.bg_surface));
        f.render_widget(paragraph, area);
    } else if app.search_active {
        let spans = vec![
            Span::styled(
                format!("Search: \"{}\"", app.search_query),
                base_style.fg(theme.accent_primary),
            ),
            Span::styled(
                format!(" \u{00b7} {} results", app.filtered_ports.len()),
                base_style.fg(theme.fg_muted),
            ),
        ];
        let paragraph = Paragraph::new(Text::from(Line::from(spans))).style(base_style);
        f.render_widget(paragraph, area);
    } else if app.filter_active || app.filter_applied {
        let spans = vec![
            Span::styled(
                format!(
                    "Filtered: {} of {} ports",
                    app.filtered_ports.len(),
                    app.ports.len()
                ),
                base_style.fg(theme.status_warning),
            ),
            Span::styled(
                " \u{00b7} combined filter active",
                base_style.fg(theme.fg_muted),
            ),
        ];
        let paragraph = Paragraph::new(Text::from(Line::from(spans))).style(base_style);
        f.render_widget(paragraph, area);
    } else if let Some(ref ks) = app.kill_status {
        // Kill outcome / progress (D-04) — 8 locked UI-SPEC strings
        let tone_style = match ks.tone {
            KillTone::InProgress => theme.fg_default,
            KillTone::Info => theme.status_info,
            KillTone::Success => theme.status_success,
            KillTone::Error => theme.status_error,
        };
        let spans = vec![Span::styled(
            ks.text.clone(),
            Style::default().fg(tone_style).bg(theme.bg_surface),
        )];
        let paragraph = Paragraph::new(Text::from(Line::from(spans))).style(base_style);
        f.render_widget(paragraph, area);
    } else {
        let now = chrono::Local::now().format("%H:%M:%S");
        // Phase 3 (SCAN-05): the live label shows the refresh mode — the 2s
        // poller today, "Live (ETW)" once the ETW change-trigger lands.
        let live_label = match app.live_mode {
            LiveMode::Polling => "Live (polling)",
            LiveMode::Etw => "Live (ETW)",
        };
        let spans = vec![
            Span::styled(live_label, base_style.fg(theme.fg_emphasis)),
            Span::styled(
                format!(" \u{00b7} {} ports", app.ports.len()),
                base_style.fg(theme.fg_default),
            ),
            Span::styled(
                format!(" \u{00b7} {}", now),
                base_style.fg(theme.fg_muted),
            ),
            admin_suffix,
        ];
        let paragraph = Paragraph::new(Text::from(Line::from(spans))).style(base_style);
        f.render_widget(paragraph, area);
    }
}

/// Render the footer with context-sensitive keyboard shortcuts per UI-SPEC.
fn render_footer(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let muted = Style::default().fg(theme.fg_muted);
    let accent = Style::default()
        .fg(theme.accent_primary)
        .add_modifier(Modifier::UNDERLINED);

    // Context-sensitive footer per UI-SPEC
    if app.search_active {
        // Search mode footer
        let line = Line::from(vec![
            Span::styled("[Esc]", accent),
            Span::styled("Cancel", muted),
            Span::styled(" ", muted),
            Span::styled("[Enter]", accent),
            Span::styled("Confirm", muted),
            Span::styled("  \u{2014}  fuzzy search across all fields", muted),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else if app.filter_active {
        // Filter mode footer
        let line = Line::from(vec![
            Span::styled("[Esc]", accent),
            Span::styled("Cancel", muted),
            Span::styled(" ", muted),
            Span::styled("[Tab]", accent),
            Span::styled("Next field", muted),
            Span::styled(" ", muted),
            Span::styled("[Enter]", accent),
            Span::styled("Apply", muted),
            Span::styled("  \u{2014}  filter by port/PID/process/state/protocol", muted),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else if app.filter_applied {
        // Filter latched (Enter applied, panel closed)
        let line = Line::from(vec![
            Span::styled("[Esc]", accent),
            Span::styled("Clear filter", muted),
            Span::styled("  ", muted),
            Span::styled("[f]", accent),
            Span::styled("Edit filter", muted),
            Span::styled(format!(
                "  \u{2014}  {} of {} ports matched",
                app.filtered_ports.len(),
                app.ports.len()
            ), muted),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else if app.whitelist_active {
        // Whitelist overlay footer (UI-SPEC Footer table — locked string):
        // "[j/k]Move [d]Delete [Tab]Focus [Enter]Add [Esc]Close"
        let line = Line::from(vec![
            Span::styled("[j/k]Move", accent),
            Span::styled(" ", muted),
            Span::styled("[d]Delete", accent),
            Span::styled(" ", muted),
            Span::styled("[Tab]Focus", accent),
            Span::styled(" ", muted),
            Span::styled("[Enter]Add", accent),
            Span::styled(" ", muted),
            Span::styled("[Esc]Close", accent),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else if app.confirm_pid.is_some() {
        // Confirm dialog footer (UI-SPEC Footer table — locked string)
        let kill_accent = Style::default()
            .fg(theme.accent_secondary)
            .add_modifier(Modifier::UNDERLINED);
        let muted_underline = Style::default()
            .fg(theme.fg_muted)
            .add_modifier(Modifier::UNDERLINED);
        let name = app
            .confirm_name
            .clone()
            .unwrap_or_else(|| "process".to_string());
        // {name} truncates with U+2026 to term_width - 63 (UI-SPEC footer table)
        let name_budget = area.width.saturating_sub(63) as usize;
        let display_name = truncate_footer_name(&name, name_budget);
        let line = Line::from(vec![
            Span::styled("[y]Confirm kill", kill_accent),
            Span::styled(" ", muted),
            Span::styled("[n]Cancel", muted_underline),
            Span::styled(
                format!(
                    "  \u{2014}  {} is on your protection list",
                    display_name
                ),
                muted,
            ),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else if app.detail_active {
        // Detail panel footer (UI-SPEC Footer table — locked string):
        // "[Esc]Close [j/k]Next port [x]Kill [r]Refresh  —  detail for {name}"
        // Fixed prefix 66 cols (UI-SPEC budget); {name} truncates with U+2026
        // to term_width - 66 — never wraps, never exceeds term_width.
        let kill_accent = Style::default()
            .fg(theme.accent_secondary)
            .add_modifier(Modifier::UNDERLINED);
        let name = app
            .selected_connection()
            .map(|c| c.process.name.clone())
            .unwrap_or_default();
        let name_budget = area.width.saturating_sub(66) as usize;
        let display_name = truncate_footer_name(&name, name_budget);
        let line = Line::from(vec![
            Span::styled("[Esc]Close", accent),
            Span::styled(" ", muted),
            Span::styled("[j/k]Next port", accent),
            Span::styled(" ", muted),
            Span::styled("[x]Kill", kill_accent),
            Span::styled(" ", muted),
            Span::styled("[r]Refresh", accent),
            Span::styled("  \u{2014}  detail for ", muted),
            Span::styled(display_name, muted),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else if app.active_tab == 1 {
        // Ports tab default footer — UI-SPEC locked string (73 cols):
        // [jk]Move [/]Search [f]Filter [d]Detail [x]Kill [r]Refresh [q]Quit [?]Help
        // [s]Sort and [a]Elevate dropped from the Ports footer (D-09; both stay
        // bound and are documented in the Help overlay, plan 02-03).
        let kill_accent = Style::default()
            .fg(theme.accent_secondary)
            .add_modifier(Modifier::UNDERLINED);
        let line = Line::from(vec![
            Span::styled("[jk]Move", accent),
            Span::styled(" ", muted),
            Span::styled("[/]Search", accent),
            Span::styled(" ", muted),
            Span::styled("[f]Filter", accent),
            Span::styled(" ", muted),
            Span::styled("[d]Detail", accent),
            Span::styled(" ", muted),
            Span::styled("[x]Kill", kill_accent),
            Span::styled(" ", muted),
            Span::styled("[r]Refresh", accent),
            Span::styled(" ", muted),
            Span::styled("[q]Quit", accent),
            Span::styled(" ", muted),
            Span::styled("[?]Help", accent),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    } else {
        // Other tabs keep the Phase 1 footer ([s]Sort + conditional [a]Elevate)
        let mut spans: Vec<Span> = vec![
            Span::styled("[\u{2191}\u{2193}jk]", accent),
            Span::styled("Navigate", muted),
            Span::styled("  ", muted),
            Span::styled("[/]", accent),
            Span::styled("Search", muted),
            Span::styled("  ", muted),
            Span::styled("[f]", accent),
            Span::styled("Filter", muted),
            Span::styled("  ", muted),
            Span::styled("[s]", accent),
            Span::styled("Sort", muted),
            Span::styled("  ", muted),
            Span::styled("[r]", accent),
            Span::styled("Refresh", muted),
        ];

        // Add elevation hint when not admin (after admin check completes)
        if !app.is_admin && app.admin_check_done {
            spans.push(Span::styled("  ", muted));
            spans.push(Span::styled("[a]", accent));
            spans.push(Span::styled("Elevate", muted));
        }

        spans.push(Span::styled("  ", muted));
        spans.push(Span::styled("[q]", accent));
        spans.push(Span::styled("Quit", muted));
        spans.push(Span::styled("  ", muted));
        spans.push(Span::styled("[?]", accent));
        spans.push(Span::styled("Help", muted));
        let footer = Paragraph::new(Text::from(Line::from(spans)))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    }
}

/// Truncate a footer-embedded name to max_len chars, appending U+2026.
/// Never wraps, never exceeds the declared budget (UI-SPEC truncation rule).
fn truncate_footer_name(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// The confirm dialog is the topmost overlay, so its keys must win even
    /// when the search bar and/or filter panel are also active. Regression for
    /// the stuck-modal ordering bug: search/filter used to dispatch before the
    /// confirm block, swallowing y/n/Enter after 'x' on a protected row.
    #[test]
    fn confirm_dispatch_beats_search_and_filter() {
        let mut app = App::new();
        app.confirm_pid = Some(42);
        app.search_active = true;
        app.filter_active = true;

        assert!(
            matches!(
                map_key_event(key(KeyCode::Char('y')), &app),
                Some(Message::KillConfirmed { pid: 42 })
            ),
            "confirm 'y' must be reachable with search+filter active"
        );
        assert!(matches!(
            map_key_event(key(KeyCode::Char('n')), &app),
            Some(Message::KillCancelled)
        ));
        assert!(matches!(
            map_key_event(key(KeyCode::Esc), &app),
            Some(Message::KillCancelled)
        ));
        // 'x' while a dialog is open is a no-op (prevents re-triggering kill).
        assert!(map_key_event(key(KeyCode::Char('x')), &app).is_none());
    }

    /// Without a dialog, printable chars reach the search input as before —
    /// the hoist must not change normal search dispatch.
    #[test]
    fn search_keys_work_when_no_confirm_dialog() {
        let mut app = App::new();
        app.confirm_pid = None;
        app.search_active = true;
        assert!(matches!(
            map_key_event(key(KeyCode::Char('y')), &app),
            Some(Message::SearchInput('y'))
        ));
    }

    /// A confirm dialog still lets non-confirm keys (e.g. universal commands)
    /// pass through to the table, per the UI-SPEC confirm pass-through table.
    #[test]
    fn confirm_passes_through_other_keys() {
        let mut app = App::new();
        app.confirm_pid = Some(7);
        app.search_active = false;
        // 'r' (refresh) is a universal default-dispatch key, not consumed by the
        // dialog — it must still produce its Refresh message.
        assert!(matches!(
            map_key_event(key(KeyCode::Char('r')), &app),
            Some(Message::Refresh)
        ));
    }
}
