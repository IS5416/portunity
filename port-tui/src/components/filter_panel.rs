//! Filter panel overlay component.
//!
//! Renders a multi-field filter panel triggered by the 'f' key.
//! Users can tab between fields, type characters into a raw text buffer
//! (accumulated per-field in App::filter_field_text), and apply or cancel.
//! Enter parses the buffer into active_filter, applies, and closes the panel.
//! Esc discards and closes.

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
        // 7 rows: header + 5 fields + help
        let rows = Layout::vertical([
            Constraint::Length(1), // header
            Constraint::Length(1), // port range
            Constraint::Length(1), // process name
            Constraint::Length(1), // pid
            Constraint::Length(1), // protocol
            Constraint::Length(1), // state
            Constraint::Length(1), // help hint
        ])
        .split(area);

        for row_area in rows.iter() {
            f.render_widget(Clear, *row_area);
        }

        let base = Style::default().bg(theme.bg_overlay);

        // Row 0: header
        let header_line = Line::from(vec![
            Span::styled(
                "Filter",
                Style::default()
                    .fg(theme.fg_emphasis)
                    .add_modifier(Modifier::BOLD)
                    .bg(theme.bg_overlay),
            ),
            Span::styled(
                "  —  [Tab]Next [Shift+Tab]Prev [Enter]Apply [Esc]Cancel",
                Style::default().fg(theme.fg_muted).bg(theme.bg_overlay),
            ),
        ]);
        f.render_widget(Paragraph::new(Text::from(header_line)).style(base), rows[0]);

        // Row 1: Port Min / Port Max (combined)
        let port_spans = render_field_row(
            "Port: ",
            FilterField::PortMin,
            app,
            theme,
        );
        f.render_widget(
            Paragraph::new(Text::from(Line::from(port_spans))).style(base),
            rows[1],
        );

        // Row 2: Process Name
        let proc_spans = render_field_row("Process: ", FilterField::ProcessName, app, theme);
        f.render_widget(
            Paragraph::new(Text::from(Line::from(proc_spans))).style(base),
            rows[2],
        );

        // Row 3: PID
        let pid_spans = render_field_row("PID: ", FilterField::Pid, app, theme);
        f.render_widget(
            Paragraph::new(Text::from(Line::from(pid_spans))).style(base),
            rows[3],
        );

        // Row 4: Protocol
        let proto_spans = render_field_row("Protocol: ", FilterField::Protocol, app, theme);
        f.render_widget(
            Paragraph::new(Text::from(Line::from(proto_spans))).style(base),
            rows[4],
        );

        // Row 5: State
        let state_spans = render_field_row("State: ", FilterField::State, app, theme);
        f.render_widget(
            Paragraph::new(Text::from(Line::from(state_spans))).style(base),
            rows[5],
        );

        // Row 6: help hint
        let help = Span::styled(
            "[Esc]Cancel [Tab]Next [Enter]Apply  —  filter by port/PID/process/state/protocol",
            Style::default().fg(theme.fg_muted).bg(theme.bg_overlay),
        );
        f.render_widget(
            Paragraph::new(Text::from(Line::from(help)))
                .alignment(Alignment::Center)
                .style(base),
            rows[6],
        );
    }
}

/// Render a labeled filter field row with focus highlighting.
fn render_field_row<'a>(
    label: &'a str,
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

    // Use raw buffer text if present, otherwise fall back to parsed filter value
    let display = field_buffer_display(field, app);
    let display_str: String = if display.is_empty() {
        "—".to_string()
    } else {
        display
    };

    vec![
        Span::styled(label, label_style),
        Span::styled(display_str, val_style),
    ]
}

/// Get the display text for a filter field: raw buffer > parsed value > empty.
fn field_buffer_display(field: FilterField, app: &App) -> String {
    // Raw text buffer takes priority (user is currently typing)
    if let Some(text) = app.filter_field_text.get(&field) {
        if !text.is_empty() {
            return text.clone();
        }
    }
    // Fall back to parsed active_filter value for display
    match field {
        FilterField::PortMin => app
            .active_filter
            .port_range
            .map(|(min, _)| min.to_string())
            .unwrap_or_default(),
        FilterField::PortMax => app
            .active_filter
            .port_range
            .map(|(_, max)| max.to_string())
            .unwrap_or_default(),
        FilterField::ProcessName => app
            .active_filter
            .process_names
            .first()
            .cloned()
            .unwrap_or_default(),
        FilterField::Pid => app
            .active_filter
            .pids
            .first()
            .map(|p| p.to_string())
            .unwrap_or_default(),
        FilterField::Protocol => app
            .active_filter
            .protocols
            .first()
            .map(|p| format!("{:?}", p))
            .unwrap_or_default(),
        FilterField::State => app
            .active_filter
            .states
            .first()
            .map(|s| format!("{:?}", s))
            .unwrap_or_default(),
    }
}
