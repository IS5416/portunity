//! Kill confirmation dialog component (D-09/D-11, UI-SPEC §Confirm Dialog).
//!
//! Centered 60x7 bordered popup, always topmost in the overlay stack.
//! Stateless renderer — all state lives in `App` (confirm_pid/name/port).
//! Keyboard: y/Enter confirm, n/Esc cancel, x intercepted as no-op (main.rs).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Kill confirmation dialog component — stateless renderer.
pub struct KillConfirmComponent;

impl Component for KillConfirmComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        // Clear the popup area first (overlay pattern — overwrites the table)
        f.render_widget(Clear, area);

        // Bordered popup with title (UI-SPEC Confirm dialog copy)
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(theme.bg_overlay))
            .title(Line::from(Span::styled(
                "Confirm termination",
                Style::default()
                    .fg(theme.fg_emphasis)
                    .add_modifier(Modifier::BOLD),
            )));

        let inner = block.inner(area);

        // Row 1: "{name} (PID {pid})" — name truncates with U+2026
        //   budget: area.width - 13 (covers " (PID " + pid digits + ")")
        let name = app.confirm_name.as_deref().unwrap_or("process");
        let pid = app.confirm_pid.unwrap_or(0);
        let name_budget = area.width.saturating_sub(13) as usize;
        let display_name = truncate(name, name_budget);

        // Row 2: plain-language reason (UI-SPEC)
        let port = app
            .confirm_port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "?".to_string());
        let reason_name = truncate(name, name_budget);
        let reason = format!(
            "{} is on your protection list. Terminating it will stop the program using port {}.",
            reason_name, port
        );

        // Row 3: buttons — [y] Confirm kill (accent_secondary, kill-action
        // highlight ONLY) · [n] Cancel (muted)
        let btn_confirm = Style::default()
            .fg(theme.accent_secondary)
            .add_modifier(Modifier::UNDERLINED)
            .bg(theme.bg_overlay);
        let btn_cancel = Style::default()
            .fg(theme.fg_muted)
            .add_modifier(Modifier::UNDERLINED)
            .bg(theme.bg_overlay);
        let muted = Style::default().fg(theme.fg_muted).bg(theme.bg_overlay);

        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

        let base = Style::default().bg(theme.bg_overlay);

        // Row 0: "{name} (PID {pid})"
        let line1 = Line::from(vec![Span::styled(
            format!("{} (PID {})", display_name, pid),
            Style::default().fg(theme.fg_default).bg(theme.bg_overlay),
        )]);
        f.render_widget(Paragraph::new(Text::from(line1)).style(base), rows[0]);

        // Row 1: plain-language reason
        let line2 = Line::from(vec![Span::styled(
            reason,
            Style::default().fg(theme.fg_default).bg(theme.bg_overlay),
        )]);
        f.render_widget(Paragraph::new(Text::from(line2)).style(base), rows[1]);

        // Row 2: buttons
        let line3 = Line::from(vec![
            Span::styled("[y] Confirm kill", btn_confirm),
            Span::styled(" \u{00b7} ", muted),
            Span::styled("[n] Cancel", btn_cancel),
        ]);
        f.render_widget(Paragraph::new(Text::from(line3)).style(base), rows[2]);

        // Render the bordered block last so borders stay on top
        f.render_widget(block, area);
    }
}

/// Truncate a string to max_len chars, appending U+2026 if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    } else {
        s.to_string()
    }
}
