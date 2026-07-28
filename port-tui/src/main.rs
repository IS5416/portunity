//! Portunity TUI — Terminal port management dashboard.
//!
//! Tab-based Widget Dashboard (TEA architecture):
//!   [1] Overview  [2] Ports  [3] History  [4] Traffic  [5] Firewall
//!
//! Plan 01-03: interactive fuzzy search ('/'), multi-dimension filter panel ('f'),
//! and admin elevation ('a') with context-sensitive status bar and footer.

mod app;
mod components;
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
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use app::App;
use components::{Component, FilterPanelComponent, PortsComponent, SearchComponent};
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
                let msg = map_key_event(key, app);
                if let Some(m) = msg {
                    update(app, m);
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
                // Shift+Tab: cycle backward (send FilterTabField; we reverse by sending 5 forw cycles)
                return Some(Message::FilterTabField);
            }
            KeyCode::Char(ch) => {
                if !ch.is_control() {
                    return Some(Message::FilterUpdateField(
                        app.filter_focused_field.clone(),
                        ch.to_string(),
                    ));
                }
            }
            KeyCode::Backspace => {
                return Some(Message::FilterUpdateField(
                    app.filter_focused_field.clone(),
                    String::new(),
                ));
            }
            // Pass-through: j, k, r, s continue to work
            _ => {}
        }
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
        KeyCode::Esc => {
            if app.error.is_some() {
                None
            } else {
                Some(Message::Quit)
            }
        }
        _ => None,
    }
}

/// Render the full application frame.
fn render_app(f: &mut Frame, app: &App, theme: &Theme) {
    let area = f.area();

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

    // Tab bar
    render_tab_bar(f, tab_bar_area, theme);

    // Adjust content area for overlays: search (3 rows), filter (5 rows) stack at top
    let overlay_offset = if app.search_active { 3u16 } else { 0u16 }
        + if app.filter_active { 5u16 } else { 0u16 };

    let table_area = if overlay_offset > 0 && content_area.height > overlay_offset {
        Rect {
            y: content_area.y + overlay_offset,
            height: content_area.height.saturating_sub(overlay_offset),
            ..content_area
        }
    } else {
        content_area
    };

    // Content: Ports component (below overlays)
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
            height: 5,
            ..content_area
        };
        FilterPanelComponent.render(app, f, filter_overlay, theme);
    }

    // Status bar
    render_status_bar(f, status_bar_area, app, theme);

    // Footer
    render_footer(f, footer_area, app, theme);
}

/// Render the tab bar — static for tracer (no active tab state yet).
fn render_tab_bar(f: &mut Frame, area: Rect, theme: &Theme) {
    let tabs = Paragraph::new(Text::from(
        " [1] Overview  [2] Ports  [3] History  [4] Traffic  [5] Firewall",
    ))
    .style(
        Style::default()
            .fg(theme.fg_muted)
            .bg(theme.bg_surface),
    );
    f.render_widget(tabs, area);
}

/// Render the status bar with context-sensitive message.
fn render_status_bar(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let (status, style) = if app.scanning {
        (
            format!(
                "Scanning... \u{00b7} {} found so far \u{00b7} {}",
                app.ports.len(),
                chrono::Local::now().format("%H:%M:%S")
            ),
            Style::default()
                .fg(theme.fg_default)
                .bg(theme.bg_surface),
        )
    } else if let Some(ref e) = app.error {
        (
            format!("\u{26a0} {} \u{00b7} Press r to retry", e),
            Style::default()
                .fg(theme.status_error)
                .bg(theme.bg_surface),
        )
    } else if app.search_active {
        // Search takes precedence in status bar
        (
            format!(
                "Search: \"{}\" \u{00b7} {} results",
                app.search_query,
                app.filtered_ports.len()
            ),
            Style::default()
                .fg(theme.accent_primary)
                .bg(theme.bg_surface),
        )
    } else if app.filter_active {
        (
            format!(
                "Filtered: {} of {} ports \u{00b7} combined filter active",
                app.filtered_ports.len(),
                app.ports.len()
            ),
            Style::default()
                .fg(theme.status_warning)
                .bg(theme.bg_surface),
        )
    } else {
        let now = chrono::Local::now().format("%H:%M:%S");
        (
            format!("Live \u{00b7} {} ports \u{00b7} {}", app.ports.len(), now),
            Style::default()
                .fg(theme.fg_default)
                .bg(theme.bg_surface),
        )
    };

    let paragraph = Paragraph::new(Text::from(status)).style(style);
    f.render_widget(paragraph, area);
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
    } else {
        // Default footer
        let line = Line::from(vec![
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
            Span::styled("  ", muted),
            Span::styled("[q]", accent),
            Span::styled("Quit", muted),
            Span::styled("  ", muted),
            Span::styled("[?]", accent),
            Span::styled("Help", muted),
        ]);
        let footer = Paragraph::new(Text::from(line))
            .style(Style::default().bg(theme.bg_surface))
            .centered();
        f.render_widget(footer, area);
    }
}
