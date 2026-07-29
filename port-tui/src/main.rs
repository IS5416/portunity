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

use app::App;
use components::{
    Component, FilterPanelComponent, FirewallTabComponent, HistoryTabComponent,
    OverviewComponent, PortsComponent, SearchComponent, TrafficTabComponent,
};
use message::Message;
use theme::Theme;
use update::update;

use port_core::scanner::PortScanner;

/// Auto-refresh interval in seconds (D-11).
const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<()> {
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

/// Run the TEA event loop with auto-refresh and keyboard navigation.
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    theme: &Theme,
    tx: &tokio::sync::mpsc::UnboundedSender<Message>,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Message>,
    scan_spawned: &mut bool,
) -> Result<()> {
    loop {
        // Render current state
        terminal.draw(|f| render_app(f, app, theme))?;

        if app.should_quit {
            break;
        }

        // Poll for keyboard events with timeout (D-10)
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events — skip Release/Repeat
                // to prevent double-firing of all keyboard actions.
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                let msg = map_key_event(key, app);
                if let Some(m) = msg {
                    // Intercept ElevateRequest: spawn blocking elevation task
                    if matches!(m, Message::ElevateRequest) {
                        if !app.elevating {
                            app.elevating = true;
                            let tx_elevate = tx.clone();
                            tokio::task::spawn_blocking(move || {
                                match elevate::elevate_to_admin() {
                                    Ok(()) => {
                                        // User declined UAC — continue in non-admin mode
                                        let _ = tx_elevate.send(Message::ElevateDeclined);
                                    }
                                    Err(e) => {
                                        // Elevation failed with an unexpected error
                                        let _ = tx_elevate.send(Message::ScanError(
                                            format!("Elevation failed: {}", e),
                                        ));
                                    }
                                }
                            });
                        }
                    } else {
                        update(app, m);
                    }
                }
            }
        }

        // Drain async channel (D-12: try_recv on each tick)
        while let Ok(msg) = rx.try_recv() {
            if matches!(msg, Message::ScanComplete(_)) {
                *scan_spawned = false;
            }
            update(app, msg);
        }

        // Spawn scan if scanning flag is set and we haven't spawned one yet
        if app.scanning && !*scan_spawned {
            spawn_scan(tx.clone());
            *scan_spawned = true;
        }

        // Auto-refresh (D-11): trigger every 5 seconds when idle
        if !app.scanning && app.error.is_none() {
            if let Some(last) = app.last_auto_refresh {
                if last.elapsed() >= AUTO_REFRESH_INTERVAL {
                    app.scanning = true;
                    spawn_scan(tx.clone());
                    *scan_spawned = true;
                }
            }
        }
    }

    Ok(())
}

/// Map a crossterm KeyEvent to an optional Message.
///
/// Handles mode-specific key dispatch: search mode, filter mode, and default mode.
/// All non-overlay keys (r, q, j, k, s, etc.) pass through when search/filter is active.
fn map_key_event(key: crossterm::event::KeyEvent, app: &App) -> Option<Message> {
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
        KeyCode::Char('r') => Some(Message::Refresh),
        KeyCode::Char('s') => Some(Message::Sort(app.sort_column)),
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
    } else {
        let now = chrono::Local::now().format("%H:%M:%S");
        let spans = vec![
            Span::styled("Live", base_style.fg(theme.fg_emphasis)),
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
    } else {
        // Default footer
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
