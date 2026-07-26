//! Portunity TUI — Terminal port management dashboard.
//!
//! Tab-based Widget Dashboard (TEA architecture):
//!   [1] Overview  [2] Ports  [3] History  [4] Traffic  [5] Firewall
//!
//! Walking skeleton for Phase 1: renders live TCP port table
//! with keyboard refresh and clean exit.

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
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use app::App;
use components::{Component, PortsComponent};
use message::Message;
use theme::Theme;
use update::update;

use port_core::scanner::PortScanner;

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

/// Run the TEA event loop.
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
                let msg = match key.code {
                    KeyCode::Char('q') => Message::Quit,
                    KeyCode::Char('r') => Message::Refresh,
                    KeyCode::Esc => Message::Quit,
                    _ => continue,
                };
                update(app, msg);
            }
        }

        // Drain async channel (D-12: try_recv on each tick)
        while let Ok(msg) = rx.try_recv() {
            update(app, msg);
            *scan_spawned = false;
        }

        // Spawn scan if scanning flag is set and we haven't spawned one yet
        if app.scanning && !*scan_spawned {
            spawn_scan(tx.clone());
            *scan_spawned = true;
        }
    }

    Ok(())
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

    // Content: Ports component
    PortsComponent.render(app, f, content_area, theme);

    // Status bar
    render_status_bar(f, status_bar_area, app, theme);

    // Footer
    render_footer(f, footer_area, theme);
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
    let status = if app.scanning {
        "Scanning...".to_string()
    } else if let Some(ref e) = app.error {
        format!("\u{26a0} {} \u{00b7} Press r to retry", e)
    } else {
        let now = chrono::Local::now().format("%H:%M:%S");
        format!(
            "Live \u{00b7} {} ports \u{00b7} {}",
            app.ports.len(),
            now
        )
    };

    let paragraph = Paragraph::new(Text::from(status)).style(
        Style::default()
            .fg(theme.fg_default)
            .bg(theme.bg_surface),
    );
    f.render_widget(paragraph, area);
}

/// Render the footer with keyboard shortcuts.
fn render_footer(f: &mut Frame, area: Rect, theme: &Theme) {
    let footer = Paragraph::new(Text::from("[r]Refresh [q]Quit"))
        .style(
            Style::default()
                .fg(theme.fg_muted)
                .bg(theme.bg_surface),
        )
        .centered();
    f.render_widget(footer, area);
}
