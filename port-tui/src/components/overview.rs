//! Overview tab component — system-wide port summary dashboard.
//!
//! Renders three sections per UI-SPEC:
//! - Port Summary (left) + Connection States (right) — top 40%
//! - Top Ports mini-table — middle 45%
//! - Admin Status card — bottom 15%
//!
//! Handles scanning, empty, and error states per copywriting contract.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use port_core::models::{PortState, Protocol};

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Overview tab component — stateless renderer.
pub struct OverviewComponent;

/// Mini-table column width for top ports list.
const MINI_COL_PORT: u16 = 7;
const MINI_COL_STATE: u16 = 6;
const MINI_COL_PROTO: u16 = 6;
const MINI_COL_PID: u16 = 6;
const MINI_COL_PROCESS: u16 = 18;

impl Component for OverviewComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        let bg = Style::default().bg(theme.bg_base);

        // Scanning state
        if app.scanning && app.ports.is_empty() {
            let text = Text::from(vec![
                Line::from(Span::styled(
                    "Scanning ports...",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_base),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Please wait",
                    Style::default()
                        .fg(theme.fg_muted)
                        .bg(theme.bg_base)
                        .add_modifier(Modifier::DIM),
                )),
            ]);
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(bg);
            f.render_widget(paragraph, area);
            return;
        }

        // Error state (no data cached)
        if let Some(ref err) = app.error {
            if app.ports.is_empty() {
                let text = Text::from(vec![
                    Line::from(Span::styled(
                        format!("\u{26a0} Scan failed: {}", err),
                        Style::default().fg(theme.status_error).bg(theme.bg_base),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press r to retry",
                        Style::default().fg(theme.fg_muted).bg(theme.bg_base),
                    )),
                ]);
                let paragraph = Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .style(bg);
                f.render_widget(paragraph, area);
                return;
            }
            // Fall through: show cached data even when error is set
        }

        // Layout: top (port summary + connection states), middle (top ports), bottom (admin)
        let sections = Layout::vertical([
            Constraint::Percentage(40), // top: summary + states
            Constraint::Percentage(45), // middle: top ports
            Constraint::Percentage(15), // bottom: admin status
        ])
        .split(area);

        let top_area = sections[0];
        let middle_area = sections[1];
        let bottom_area = sections[2];

        // Top section: horizontal split (left=Port Summary, right=Connection States)
        let top_split = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(top_area);

        let summary_area = top_split[0];
        let states_area = top_split[1];

        // Render sub-panels
        render_port_summary(f, summary_area, app, theme);
        render_connection_states(f, states_area, app, theme);
        render_top_ports(f, middle_area, app, theme);
        render_admin_status(f, bottom_area, app, theme);
    }
}

/// Render the Port Summary panel: total ports, TCP/UDP counts, IPv4/IPv6 counts.
fn render_port_summary(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg_muted))
        .title(Span::styled(
            "Port Summary",
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme.bg_surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Compute stats from app.ports (unfiltered full list)
    let total = app.ports.len();
    let tcp_count = app
        .ports
        .iter()
        .filter(|c| matches!(c.port.protocol, Protocol::Tcp | Protocol::Tcp6))
        .count();
    let udp_count = app
        .ports
        .iter()
        .filter(|c| matches!(c.port.protocol, Protocol::Udp | Protocol::Udp6))
        .count();
    let ipv4_count = app
        .ports
        .iter()
        .filter(|c| matches!(c.port.protocol, Protocol::Tcp | Protocol::Udp))
        .count();
    let ipv6_count = app
        .ports
        .iter()
        .filter(|c| matches!(c.port.protocol, Protocol::Tcp6 | Protocol::Udp6))
        .count();

    let label_style = Style::default().fg(theme.fg_muted).bg(theme.bg_surface);
    let value_style = Style::default()
        .fg(theme.fg_default)
        .bg(theme.bg_surface)
        .add_modifier(Modifier::BOLD);

    let rows = vec![
        ("Total:", total),
        ("TCP:", tcp_count),
        ("UDP:", udp_count),
        ("IPv4:", ipv4_count),
        ("IPv6:", ipv6_count),
    ];

    let lines: Vec<Line> = rows
        .into_iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(
                    format!("  {:<10}", label),
                    label_style,
                ),
                Span::styled(
                    format!("{:>6}", value),
                    value_style,
                ),
            ])
        })
        .collect();

    // If scanning, add a dim note about stale data
    let mut text_lines = lines;
    if app.scanning {
        text_lines.push(Line::from(Span::styled(
            "  (refreshing...)",
            Style::default()
                .fg(theme.fg_muted)
                .bg(theme.bg_surface)
                .add_modifier(Modifier::DIM),
        )));
    }

    let paragraph = Paragraph::new(Text::from(text_lines))
        .style(Style::default().bg(theme.bg_surface));
    f.render_widget(paragraph, inner);
}

/// Render the Connection States panel: per-state counts with colored symbols.
fn render_connection_states(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg_muted))
        .title(Span::styled(
            "Connection States",
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme.bg_surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Count ports by state
    let listening = app
        .ports
        .iter()
        .filter(|c| c.port.state == PortState::Listen)
        .count();
    let established = app
        .ports
        .iter()
        .filter(|c| c.port.state == PortState::Established)
        .count();
    let time_wait = app
        .ports
        .iter()
        .filter(|c| c.port.state == PortState::TimeWait)
        .count();
    let close_wait = app
        .ports
        .iter()
        .filter(|c| c.port.state == PortState::CloseWait)
        .count();
    let syn_sent = app.ports.iter().filter(|c| c.port.state == PortState::SynSent).count();

    struct StateRow {
        symbol: &'static str,
        label: &'static str,
        count: usize,
        color: ratatui::style::Color,
    }

    let state_rows = vec![
        StateRow { symbol: "\u{25cf}", label: "Listening", count: listening, color: theme.status_success },
        StateRow { symbol: "\u{25cf}", label: "Established", count: established, color: theme.status_info },
        StateRow { symbol: "\u{25cb}", label: "Time Wait", count: time_wait, color: theme.fg_muted },
        StateRow { symbol: "\u{25c9}", label: "Close Wait", count: close_wait, color: theme.status_warning },
        StateRow { symbol: "\u{25cf}", label: "Syn Sent", count: syn_sent, color: theme.status_error },
    ];

    let lines: Vec<Line> = state_rows
        .into_iter()
        .map(|sr| {
            Line::from(vec![
                Span::styled(
                    format!("  {} {}", sr.symbol, sr.label),
                    Style::default()
                        .fg(sr.color)
                        .bg(theme.bg_surface),
                ),
                // Spacer to push count right
                Span::styled(
                    "  ",
                    Style::default().bg(theme.bg_surface),
                ),
                Span::styled(
                    format!("{:>4}", sr.count),
                    Style::default()
                        .fg(theme.fg_default)
                        .bg(theme.bg_surface)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(theme.bg_surface));
    f.render_widget(paragraph, inner);
}

/// Render the Top Ports mini-table: top 10 ports sorted by port number.
fn render_top_ports(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg_muted))
        .title(Span::styled(
            "Top Ports",
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme.bg_surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Use display_data for consistency with active search/filter
    let display_data = app.display_data();

    if display_data.is_empty() {
        let text = if app.scanning {
            "Scanning..."
        } else {
            "No active ports"
        };
        let paragraph = Paragraph::new(Text::from(text))
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.fg_muted).bg(theme.bg_surface));
        f.render_widget(paragraph, inner);
        return;
    }

    // Take top 10 entries (they're already sorted by whatever sort is active, or natural order)
    let top_n = 10usize.min(display_data.len());

    let header_style = Style::default()
        .fg(theme.fg_emphasis)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from(Text::styled("Port", header_style)),
        Cell::from(Text::styled("State", header_style)),
        Cell::from(Text::styled("Proto", header_style)),
        Cell::from(Text::styled("PID", header_style)),
        Cell::from(Text::styled("Process", header_style)),
    ]);

    let text_style = Style::default().fg(theme.fg_default).bg(theme.bg_surface);

    let rows: Vec<Row> = display_data
        .iter()
        .take(top_n)
        .map(|conn| {
            let (symbol, label, color) = mini_state_display(conn.port.state, theme);

            Row::new(vec![
                Cell::from(Text::styled(
                    conn.port.number.to_string(),
                    text_style,
                )),
                Cell::from(Text::from(Line::from(vec![
                    Span::styled(symbol, Style::default().fg(color).bg(theme.bg_surface)),
                    Span::styled(" ", text_style),
                    Span::styled(label, text_style),
                ]))),
                Cell::from(Text::styled(
                    protocol_label(conn.port.protocol),
                    text_style,
                )),
                Cell::from(Text::styled(
                    conn.process.pid.to_string(),
                    text_style,
                )),
                Cell::from(Text::styled(
                    truncate(&conn.process.name, (MINI_COL_PROCESS - 1) as usize),
                    text_style,
                )),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(MINI_COL_PORT),
        Constraint::Length(MINI_COL_STATE + 2), // +2 for symbol+space
        Constraint::Length(MINI_COL_PROTO),
        Constraint::Length(MINI_COL_PID),
        Constraint::Length(MINI_COL_PROCESS),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .style(Style::default().bg(theme.bg_surface));

    f.render_widget(table, inner);
}

/// Render the Admin Status card.
fn render_admin_status(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg_muted))
        .title(Span::styled(
            "Admin Status",
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme.bg_surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let line = if app.admin_check_done {
        if app.is_admin {
            Line::from(vec![
                Span::styled(
                    " Admin: \u{2713} Full access",
                    Style::default()
                        .fg(theme.status_success)
                        .bg(theme.bg_surface),
                ),
                Span::styled(
                    " — all process details available",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_surface),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    " Admin: \u{2717} Limited",
                    Style::default()
                        .fg(theme.status_warning)
                        .bg(theme.bg_surface),
                ),
                Span::styled(
                    " — Press ",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_surface),
                ),
                Span::styled(
                    "[a]",
                    Style::default()
                        .fg(theme.accent_primary)
                        .bg(theme.bg_surface)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled(
                    " to elevate for full process details",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_surface),
                ),
            ])
        }
    } else {
        Line::from(Span::styled(
            " Checking admin status...",
            Style::default()
                .fg(theme.fg_muted)
                .bg(theme.bg_surface)
                .add_modifier(Modifier::DIM),
        ))
    };

    let paragraph = Paragraph::new(Text::from(line))
        .style(Style::default().bg(theme.bg_surface))
        .alignment(Alignment::Left);
    f.render_widget(paragraph, inner);
}

/// Compact state display for mini-table (abbreviated labels).
fn mini_state_display(
    state: PortState,
    theme: &Theme,
) -> (&'static str, &'static str, ratatui::style::Color) {
    match state {
        PortState::Listen => ("\u{25cf}", "LIST", theme.status_success),
        PortState::Established => ("\u{25cf}", "EST", theme.status_info),
        PortState::TimeWait => ("\u{25cb}", "T_WAIT", theme.fg_muted),
        PortState::CloseWait => ("\u{25c9}", "C_WAIT", theme.status_warning),
        PortState::SynSent => ("\u{25cf}", "SYN_S", theme.status_error),
        PortState::SynReceived => ("\u{25cb}", "SYN_R", theme.fg_muted),
        PortState::FinWait1 => ("\u{25cb}", "FIN1", theme.fg_muted),
        PortState::FinWait2 => ("\u{25cb}", "FIN2", theme.fg_muted),
        PortState::LastAck => ("\u{25cb}", "L_ACK", theme.fg_muted),
        PortState::Closing => ("\u{25cb}", "CLOSE", theme.fg_muted),
        PortState::Unknown => ("\u{2014}", "UDP", theme.fg_muted),
    }
}

/// Short protocol display label.
fn protocol_label(p: Protocol) -> &'static str {
    match p {
        Protocol::Tcp => "TCP",
        Protocol::Udp => "UDP",
        Protocol::Tcp6 => "TCP6",
        Protocol::Udp6 => "UDP6",
    }
}

/// Truncate a string to max_len, appending "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    } else {
        s.to_string()
    }
}
