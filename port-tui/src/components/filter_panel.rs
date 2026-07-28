//! Filter panel overlay component.
//!
//! Renders a multi-field filter panel triggered by the 'f' key.
//! Users can tab between fields and enter filter criteria.
//! The panel renders as a 5-row overlay below the search bar area,
//! using `Clear` to overwrite port table rows behind it.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::components::Component;
use crate::message::FilterField;
use crate::theme::Theme;

/// Filter panel component — stateless renderer.
pub struct FilterPanelComponent;

impl Component for FilterPanelComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        let panel_area = Layout::vertical([
            Constraint::Length(1), // header
            Constraint::Length(1), // port range
            Constraint::Length(1), // process name
            Constraint::Length(1), // pid/proto/state
            Constraint::Length(1), // help hint
        ])
        .split(area);

        // Clear background
        for row_area in panel_area.iter() {
            f.render_widget(Clear, *row_area);
        }

        let text_style = Style::default()
            .fg(theme.fg_default)
            .bg(theme.bg_overlay);
        let muted = Style::default()
            .fg(theme.fg_muted)
            .bg(theme.bg_overlay);
        let emphasis = Style::default()
            .fg(theme.fg_emphasis)
            .add_modifier(Modifier::BOLD)
            .bg(theme.bg_overlay);

        // Row 0: header
        let header_line = Line::from(vec![
            Span::styled("Filter", emphasis),
            Span::styled("  \u{2014}  ", muted),
            Span::styled("[Tab]Next field [Enter]Apply [Esc]Cancel", muted),
        ]);
        f.render_widget(
            Paragraph::new(Text::from(header_line)).style(Style::default().bg(theme.bg_overlay)),
            panel_area[0],
        );

        // Row 1: port range
        let port_min_str = app.active_filter.port_range
            .map(|(min, _)| min.to_string())
            .unwrap_or_default();
        let port_max_str = app.active_filter.port_range
            .map(|(_, max)| max.to_string())
            .unwrap_or_default();
        let port_range_display = format!("{}-{}", port_min_str, port_max_str);

        let port_spans = build_field_row(
            "Port: ",
            &port_range_display,
            FilterField::PortMin,
            app,
            theme,
        );
        f.render_widget(
            Paragraph::new(Text::from(Line::from(port_spans)))
                .style(Style::default().bg(theme.bg_overlay)),
            panel_area[1],
        );

        // Row 2: process name
        let proc_name = app.active_filter.process_names.first()
            .map(|s| s.as_str())
            .unwrap_or("");
        let proc_spans = build_field_row(
            "Process: ",
            proc_name,
            FilterField::ProcessName,
            app,
            theme,
        );
        f.render_widget(
            Paragraph::new(Text::from(Line::from(proc_spans)))
                .style(Style::default().bg(theme.bg_overlay)),
            panel_area[2],
        );

        // Row 3: PID, Protocol, State (combined row)
        let pid_str = app.active_filter.pids.first()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let proto_display = if app.active_filter.protocols.is_empty() {
            String::new()
        } else {
            match app.active_filter.protocols[0] {
                port_core::models::Protocol::Tcp => "TCP".to_string(),
                port_core::models::Protocol::Udp => "UDP".to_string(),
                port_core::models::Protocol::Tcp6 => "TCP6".to_string(),
                port_core::models::Protocol::Udp6 => "UDP6".to_string(),
            }
        };
        let state_display = if app.active_filter.states.is_empty() {
            String::new()
        } else {
            format!("{:?}", app.active_filter.states[0])
        };

        let pid_spans = build_field_row(
            "PID: ",
            &pid_str,
            FilterField::Pid,
            app,
            theme,
        );
        let proto_label = if proto_display.is_empty() { "—" } else { &proto_display };
        let proto_spans = vec![
            Span::styled("  Proto: ", muted),
            Span::styled(proto_label, text_style),
        ];
        let state_label = if state_display.is_empty() { "—" } else { &state_display };
        let state_spans = vec![
            Span::styled("  State: ", muted),
            Span::styled(state_label, text_style),
        ];

        let mut combined: Vec<Span> = Vec::new();
        combined.extend(pid_spans);
        combined.extend(proto_spans);
        combined.extend(state_spans);

        f.render_widget(
            Paragraph::new(Text::from(Line::from(combined)))
                .style(Style::default().bg(theme.bg_overlay)),
            panel_area[3],
        );

        // Row 4: help hint
        let help = Span::styled(
            "[Esc]Cancel [Tab]Next field [Enter]Apply  —  filter by port/PID/process/state/protocol",
            muted,
        );
        f.render_widget(
            Paragraph::new(Text::from(Line::from(help)))
                .alignment(Alignment::Center)
                .style(Style::default().bg(theme.bg_overlay)),
            panel_area[4],
        );
    }
}

/// Build a labeled field row with focus highlighting.
fn build_field_row<'a>(
    label: &'a str,
    value: &'a str,
    field: FilterField,
    app: &App,
    theme: &'a Theme,
) -> Vec<Span<'a>> {
    let muted = Style::default()
        .fg(theme.fg_muted)
        .bg(theme.bg_overlay);
    let accent = Style::default()
        .fg(theme.accent_primary)
        .add_modifier(Modifier::BOLD)
        .bg(theme.bg_overlay);
    let focus_style = Style::default()
        .fg(theme.fg_default)
        .bg(theme.bg_selection);
    let text_style = Style::default()
        .fg(theme.fg_default)
        .bg(theme.bg_overlay);

    let is_focused = app.filter_focused_field == field;

    let label_style = if is_focused { accent } else { muted };
    let val_style = if is_focused { focus_style } else { text_style };

    let display = if value.is_empty() { "—" } else { value };

    vec![
        Span::styled(label, label_style),
        Span::styled(display, val_style),
    ]
}
