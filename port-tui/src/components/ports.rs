//! Port table component — renders TCP port data as a Ratatui Table.
//!
//! Handles all visual states: scanning (spinner text), empty, error, and
//! populated with color-coded connection states.

use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use port_core::models::PortState;

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Port table component — stateless renderer.
pub struct PortsComponent;

impl Component for PortsComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default().bg(theme.bg_base));

        if app.scanning {
            let paragraph = Paragraph::new(Text::from("Scanning ports..."))
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.fg_muted).bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        if let Some(ref err) = app.error {
            let text = Text::from(format!("Scan failed: {}", err));
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.status_error).bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        if app.ports.is_empty() {
            let text = Text::from("No active ports\n\nNo TCP ports are currently in use.");
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.fg_muted).bg(theme.bg_base))
                .block(block);
            f.render_widget(paragraph, area);
            return;
        }

        // Build header
        let header_style = Style::default()
            .fg(theme.fg_emphasis)
            .add_modifier(Modifier::BOLD);
        let header = Row::new(vec![
            Cell::from("State"),
            Cell::from("Port"),
            Cell::from("Proto"),
            Cell::from("Process"),
            Cell::from("PID"),
        ])
        .style(header_style)
        .bottom_margin(0);

        // Column widths
        let widths = [
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(18),
            Constraint::Length(6),
        ];

        // Build data rows
        let rows: Vec<Row> = app
            .ports
            .iter()
            .enumerate()
            .map(|(i, conn)| {
                let (symbol, color) = state_symbol(conn.port.state, theme);
                let bg = if i % 2 == 0 {
                    theme.bg_base
                } else {
                    theme.bg_surface
                };

                let row_style = Style::default().bg(bg);
                let symbol_style = Style::default().fg(color).bg(bg);
                let text_style = Style::default().fg(theme.fg_default).bg(bg);

                Row::new(vec![
                    Cell::from(Text::styled(symbol, symbol_style)),
                    Cell::from(Text::styled(
                        conn.port.number.to_string(),
                        text_style,
                    )),
                    Cell::from(Text::styled("TCP", text_style)),
                    Cell::from(Text::styled(
                        truncate(&conn.process.name, 17),
                        text_style,
                    )),
                    Cell::from(Text::styled(
                        conn.process.pid.to_string(),
                        text_style,
                    )),
                ])
                .style(row_style)
            })
            .collect();

        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .column_spacing(1);

        f.render_widget(table, area);
    }
}

/// Map PortState to display symbol and color.
fn state_symbol(state: PortState, theme: &Theme) -> (String, ratatui::style::Color) {
    match state {
        PortState::Listen => (" ● ".to_string(), theme.status_success),
        PortState::Established => (" ● ".to_string(), theme.status_info),
        PortState::TimeWait => (" ○ ".to_string(), theme.fg_muted),
        PortState::CloseWait => (" ◉ ".to_string(), theme.status_warning),
        PortState::SynSent => (" ● ".to_string(), theme.status_error),
        _ => (" ○ ".to_string(), theme.fg_muted),
    }
}

/// Truncate a string to max_len, appending "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}
