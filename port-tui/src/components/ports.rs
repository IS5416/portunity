//! Port table component — renders TCP/UDP port data as a Ratatui Table.
//!
//! Features per UI-SPEC:
//! - State column with abbreviated text labels alongside colored symbols
//! - Full color mapping for all TCP connection states + UDP
//! - Sort indicators (▲/▼) on column headers per SCAN-04
//! - Row selection with reverse video highlight
//! - Virtual scrolling (viewport-only rendering) per TUI-04
//! - Zebra striping (bg_base / bg_surface alternating)
//! - Scrollbar on right edge
//! - Handles scanning, empty, error, and populated states

use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use port_core::models::PortState;

use crate::app::App;
use crate::components::Component;
use crate::message::{SortColumn, SortOrder};
use crate::theme::Theme;

/// Port table component — stateless renderer.
pub struct PortsComponent;

/// Column widths. State expanded to 10 per user feedback (symbol + label).
const COL_STATE: u16 = 10;
const COL_PORT: u16 = 7;
const COL_PROTO: u16 = 6;
const COL_PROCESS: u16 = 18;
const COL_PID: u16 = 6;

impl Component for PortsComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default().bg(theme.bg_base));

        // Scanning state
        if app.scanning {
            let paragraph = Paragraph::new(Text::from("Scanning ports..."))
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.fg_muted).bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        // Error state (D-03: keep last data visible, show error above)
        if let Some(ref err) = app.error {
            let text = Text::from(format!(
                "\u{26a0} Scan failed: {} \u{00b7} Press r to retry",
                err
            ));
            // Show table if we have cached data, otherwise show error
            if app.ports.is_empty() {
                let paragraph = Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(theme.status_error)
                            .bg(theme.bg_base),
                    )
                    .block(block);
                f.render_widget(paragraph, area);
                return;
            }
            // Fall through to render cached data with error status bar
        }

        // Determine which data to display
        let display_data = app.display_data();

        // Search empty state (UI-SPEC: "No ports match '{query}'")
        if app.search_active && !app.search_query.is_empty() && display_data.is_empty() {
            let text = Text::from(vec![
                Line::from(Span::styled(
                    format!("No ports match \"{}\"", app.search_query),
                    Style::default().fg(theme.fg_muted).bg(theme.bg_base),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Try adjusting your search terms or press Esc to clear.",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_base),
                )),
            ]);
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        // Filter empty state (UI-SPEC: "No matching ports")
        if app.filter_active && display_data.is_empty() {
            let text = Text::from(vec![
                Line::from(Span::styled(
                    "No matching ports",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_base),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "No ports match the current filters. Try broadening your criteria or press Esc to clear all filters.",
                    Style::default().fg(theme.fg_muted).bg(theme.bg_base),
                )),
            ]);
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        // Empty state (no ports at all)
        if display_data.is_empty() {
            let text =
                Text::from("No active ports\n\nNo TCP or UDP ports are currently in use. This is unusual \u{2014} check if network services are running.");
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.fg_muted).bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        // Calculate usable area (account for scrollbar column)
        let table_width = area.width.saturating_sub(1); // 1 col for scrollbar
        let table_area = Rect {
            width: table_width,
            ..area
        };

        // Column widths
        let widths = [
            Constraint::Length(COL_STATE),
            Constraint::Length(COL_PORT),
            Constraint::Length(COL_PROTO),
            Constraint::Length(COL_PROCESS),
            Constraint::Length(COL_PID),
        ];

        // Virtual scrolling: compute visible row range
        let header_rows: u16 = 1; // header row
        let available_rows = table_area.height.saturating_sub(header_rows).saturating_sub(2); // block borders
        let total_rows = display_data.len() as u16;

        // Compute scroll offset to keep selected row visible
        let scroll_offset = compute_scroll_offset(
            app.selected_index as u16,
            available_rows as u16,
            total_rows,
        );

        let visible_start = scroll_offset as usize;
        let visible_end = (visible_start + available_rows as usize).min(display_data.len());

        // Build header with sort indicators
        let header_style = Style::default()
            .fg(theme.fg_emphasis)
            .add_modifier(Modifier::BOLD);
        let header = Row::new(vec![
            header_cell("State", SortColumn::State, app, theme),
            header_cell("Port", SortColumn::Port, app, theme),
            header_cell("Proto", SortColumn::Protocol, app, theme),
            header_cell("Process", SortColumn::ProcessName, app, theme),
            header_cell("PID", SortColumn::Pid, app, theme),
        ])
        .style(header_style)
        .bottom_margin(0);

        // Build visible data rows
        let rows: Vec<Row> = display_data
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(visible_end - visible_start)
            .map(|(global_idx, conn)| {
                let is_selected = global_idx == app.selected_index;
                let is_even = global_idx % 2 == 0;

                let bg = if is_selected {
                    theme.bg_selection
                } else if is_even {
                    theme.bg_base
                } else {
                    theme.bg_surface
                };

                let (symbol, label, color) = state_display(conn.port.state, theme);

                // Determine process name style: dim system processes when non-admin (SCAN-07, D-09)
                let system_dim = !app.is_admin && is_system_process(&conn.process.name, conn.process.pid);

                // Selected row: reverse video overrides per-cell color
                if is_selected {
                    let rev_style = Style::default()
                        .fg(theme.fg_default)
                        .bg(theme.bg_selection)
                        .add_modifier(Modifier::REVERSED);
                    let proc_style = if system_dim {
                        rev_style.add_modifier(Modifier::DIM)
                    } else {
                        rev_style
                    };

                    Row::new(vec![
                        Cell::from(Text::styled(
                            format!("{} {}", symbol, label),
                            rev_style,
                        )),
                        Cell::from(Text::styled(
                            conn.port.number.to_string(),
                            rev_style,
                        )),
                        Cell::from(Text::styled(
                            protocol_label(conn.port.protocol),
                            rev_style,
                        )),
                        Cell::from(Text::styled(
                            truncate(&conn.process.name, (COL_PROCESS - 1) as usize),
                            proc_style,
                        )),
                        Cell::from(Text::styled(
                            conn.process.pid.to_string(),
                            rev_style,
                        )),
                    ])
                    .style(rev_style)
                } else {
                    let symbol_style = Style::default().fg(color).bg(bg);
                    let label_style = Style::default().fg(color).bg(bg);
                    let text_style = Style::default().fg(theme.fg_default).bg(bg);
                    let proc_style = if system_dim {
                        text_style.add_modifier(Modifier::DIM)
                    } else {
                        text_style
                    };

                    Row::new(vec![
                        Cell::from(Text::from(Line::from(vec![
                            Span::styled(symbol, symbol_style),
                            Span::styled(" ", text_style),
                            Span::styled(label, label_style),
                        ]))),
                        Cell::from(Text::styled(
                            conn.port.number.to_string(),
                            text_style,
                        )),
                        Cell::from(Text::styled(
                            protocol_label(conn.port.protocol),
                            text_style,
                        )),
                        Cell::from(Text::styled(
                            truncate(&conn.process.name, (COL_PROCESS - 1) as usize),
                            proc_style,
                        )),
                        Cell::from(Text::styled(
                            conn.process.pid.to_string(),
                            text_style,
                        )),
                    ])
                    .style(Style::default().bg(bg))
                }
            })
            .collect();

        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .column_spacing(1);

        f.render_widget(table, table_area);

        // Render scrollbar on the right edge
        if total_rows > available_rows {
            render_scrollbar(
                f,
                area,
                scroll_offset as u16,
                total_rows,
                available_rows,
                theme,
            );
        }
    }
}

/// Compute scroll offset to keep the selected row visible.
fn compute_scroll_offset(
    selected: u16,
    viewport_height: u16,
    total: u16,
) -> u16 {
    if total <= viewport_height {
        return 0;
    }

    let max_offset = total - viewport_height;

    if selected < viewport_height {
        0
    } else if selected >= max_offset {
        max_offset
    } else {
        // Keep selected row centered when possible
        selected.saturating_sub(viewport_height / 2).min(max_offset)
    }
}

/// Build a header cell with optional sort indicator.
fn header_cell<'a>(label: &str, col: SortColumn, app: &App, theme: &Theme) -> Cell<'a> {
    let text = if app.sort_column == col {
        match app.sort_order {
            SortOrder::Ascending => format!("{} \u{25b2}", label),   // ▲
            SortOrder::Descending => format!("{} \u{25bc}", label),  // ▼
            SortOrder::None => label.to_string(),
        }
    } else {
        label.to_string()
    };

    Cell::from(Text::styled(
        text,
        Style::default()
            .fg(theme.fg_emphasis)
            .add_modifier(Modifier::BOLD),
    ))
}

/// Map PortState to display (symbol, label, color) per UI-SPEC color map.
fn state_display(state: PortState, theme: &Theme) -> (&'static str, &'static str, Color) {
    match state {
        PortState::Listen => ("\u{25cf}", "LISTEN", theme.status_success),
        PortState::Established => ("\u{25cf}", "ESTAB", theme.status_info),
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
fn protocol_label(p: port_core::models::Protocol) -> &'static str {
    match p {
        port_core::models::Protocol::Tcp => "TCP",
        port_core::models::Protocol::Udp => "UDP",
        port_core::models::Protocol::Tcp6 => "TCP6",
        port_core::models::Protocol::Udp6 => "UDP6",
    }
}

/// Known system-owned process names for dimming when non-admin.
/// This is a simple heuristic (PID < 1000 OR name in this set), NOT the full
/// whitelist (Phase 2). It provides enough signal for non-admin grace period.
const SYSTEM_NAMES: &[&str] = &[
    "svchost.exe",
    "services.exe",
    "lsass.exe",
    "winlogon.exe",
    "csrss.exe",
    "smss.exe",
    "wininit.exe",
    "System",
    "System Idle Process",
    "Registry",
    "spoolsv.exe",
    "winlogon.exe",
];

/// Check whether a process is likely system-owned and needs admin for full details.
fn is_system_process(name: &str, pid: u32) -> bool {
    pid < 1000 || SYSTEM_NAMES.iter().any(|s| name.eq_ignore_ascii_case(s))
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

/// Render a vertical scrollbar on the rightmost column of the table area.
fn render_scrollbar(
    f: &mut Frame,
    area: Rect,
    scroll_offset: u16,
    total: u16,
    viewport: u16,
    theme: &Theme,
) {
    let scroll_area = Rect {
        x: area.right().saturating_sub(1),
        y: area.y + 1, // skip header
        width: 1,
        height: area.height.saturating_sub(1),
    };

    let track_height = scroll_area.height as usize;
    if track_height == 0 {
        return;
    }

    let thumb_size = (((viewport as f64 / total as f64) * track_height as f64)
        .ceil() as usize)
        .max(1);
    let thumb_pos = ((scroll_offset as f64 / (total - viewport) as f64)
        * (track_height - thumb_size) as f64) as usize;

    let mut lines: Vec<Line> = Vec::with_capacity(track_height);

    for i in 0..track_height {
        let ch = if i >= thumb_pos && i < thumb_pos + thumb_size {
            "\u{2588}" // █ full block for thumb
        } else {
            "\u{2502}" // │ for track
        };
        let style = Style::default().fg(theme.fg_muted).bg(theme.bg_base);
        lines.push(Line::styled(ch, style));
    }

    let paragraph = Paragraph::new(Text::from(lines));
    f.render_widget(paragraph, scroll_area);
}
