//! Whitelist overlay component (D-13..D-15, PROC-05, UI-SPEC §Whitelist
//! Overlay Internal Layout).
//!
//! 20-row top-anchored Clear-over: read-only built-in section (◆ prefix +
//! short plain-language reason per entry), selectable user list (→ prefix,
//! Reverse selection, scrollbar on overflow), path input row (search-bar
//! block-cursor pattern), hint row, bottom border.
//!
//! Stateless renderer — all state lives in `App` (whitelist_active /
//! whitelist_focus / whitelist_selected / whitelist_input /
//! whitelist_input_cursor / whitelist_settings).
//!
//! The built-in section is read-only by design (D-13): it renders the
//! port-core `BUILTIN` constant plus the PID 4/0 special rows and has no
//! add/delete path — the only mutations flow through the user list.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use port_core::process::{builtin_match, BUILTIN};

use crate::app::{App, WhitelistFocus};
use crate::components::Component;
use crate::theme::Theme;

/// Whitelist overlay component — stateless renderer.
pub struct WhitelistOverlayComponent;

impl Component for WhitelistOverlayComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        // Soft-overlay pattern: clear the surface first.
        f.render_widget(Clear, area);

        let base = Style::default().bg(theme.bg_overlay);
        let muted = Style::default().fg(theme.fg_muted).bg(theme.bg_overlay);

        // 20-row layout per UI-SPEC (title, built-in label, 9 built-in rows,
        // user label, min-5 user rows, input, hint, bottom border).
        let rows = Layout::vertical([
            Constraint::Length(1), // row 0: title
            Constraint::Length(1), // row 1: built-in label
            Constraint::Length(9), // rows 2-10: built-in list
            Constraint::Length(1), // row 11: user label
            Constraint::Min(5),    // rows 12-16: user list
            Constraint::Length(1), // row 17: path input
            Constraint::Length(1), // row 18: hint
            Constraint::Length(1), // row 19: bottom border
        ])
        .split(area);

        // Row 0: title "Protection Whitelist" (Bold fg_emphasis) + [Esc].
        let title_line = Line::from(vec![
            Span::styled(
                "Protection Whitelist",
                Style::default()
                    .fg(theme.fg_emphasis)
                    .add_modifier(Modifier::BOLD)
                    .bg(theme.bg_overlay),
            ),
            Span::styled("  [Esc]", muted),
        ]);
        f.render_widget(Paragraph::new(Text::from(title_line)).style(base), rows[0]);

        // Row 1: built-in section label.
        let builtin_label = Line::from(Span::styled(
            "Built-in — system-critical, cannot be terminated",
            muted,
        ));
        f.render_widget(Paragraph::new(Text::from(builtin_label)).style(base), rows[1]);

        // Rows 2-10: read-only built-in list (D-13 — never focusable).
        render_builtin_list(f, rows[2], theme);

        // Row 11: user section label.
        let user_label = Line::from(Span::styled(
            "User list — termination requires confirmation",
            muted,
        ));
        f.render_widget(Paragraph::new(Text::from(user_label)).style(base), rows[3]);

        // Rows 12-16: user list (selectable, scrollable).
        render_user_list(f, app, rows[4], theme);

        // Row 17: "Path: >_" input row (search.rs block-cursor pattern).
        render_input_row(f, app, rows[5], theme);

        // Row 18: hint.
        let hint_line = Line::from(Span::styled(
            "[j/k]Move [d]Delete [Tab]Focus [Enter]Add [Esc]Close",
            muted,
        ));
        f.render_widget(Paragraph::new(Text::from(hint_line)).style(base), rows[6]);

        // Row 19: bottom border (U+2500 run).
        let border = "\u{2500}".repeat(area.width as usize);
        let border_line = Line::from(Span::styled(border, muted));
        f.render_widget(Paragraph::new(Text::from(border_line)).style(base), rows[7]);
    }
}

/// Build the 9 read-only built-in rows: the PID 4/0 special-case rows first
/// (rendered from the PID entries via builtin_match), then the first 7
/// `BUILTIN` entries (UI-SPEC: 9 built-in rows at 80x24).
fn builtin_display_rows() -> Vec<(String, String)> {
    let mut rows = Vec::with_capacity(9);
    if let Some(reason) = builtin_match(4, "") {
        rows.push(("System (PID 4)".to_string(), reason.to_string()));
    }
    if let Some(reason) = builtin_match(0, "") {
        rows.push(("Idle (PID 0)".to_string(), reason.to_string()));
    }
    for entry in BUILTIN.iter().take(7) {
        rows.push((entry.name.to_string(), entry.reason.to_string()));
    }
    rows
}

/// Render the built-in list rows: "◆ {basename}  {short reason}" with the
/// diamond in status.error and the name in fg_muted Dim (read-only).
fn render_builtin_list(f: &mut Frame, area: Rect, theme: &Theme) {
    let base = Style::default().bg(theme.bg_overlay);
    let diamond = Style::default().fg(theme.status_error).bg(theme.bg_overlay);
    let name_style = Style::default()
        .fg(theme.fg_muted)
        .add_modifier(Modifier::DIM)
        .bg(theme.bg_overlay);
    let reason_style = Style::default().fg(theme.fg_default).bg(theme.bg_overlay);

    let width = area.width as usize;
    let mut lines: Vec<Line> = Vec::with_capacity(9);
    for (name, reason) in builtin_display_rows() {
        // "◆ " (2 cols) + name (capped) + "  " (2 cols) + short reason.
        let name_budget = name.chars().count().min(28);
        let reason_budget = width.saturating_sub(4 + name_budget);
        let display_name = truncate(&name, name_budget);
        let display_reason = truncate(&reason, reason_budget);
        lines.push(Line::from(vec![
            Span::styled("\u{25c6} ", diamond),
            Span::styled(display_name, name_style),
            Span::styled("  ", base),
            Span::styled(display_reason, reason_style),
        ]));
    }
    f.render_widget(Paragraph::new(Text::from(lines)).style(base), area);
}

/// Render the user list: "→ {full executable path}", selected entry
/// Reverse (list focus) / bg_selection (input focus), empty-state copy,
/// ports.rs scrollbar pattern when overflowing.
fn render_user_list(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let base = Style::default().bg(theme.bg_overlay);
    let entries = &app.whitelist_settings.whitelist;
    let viewport = area.height as usize;
    let len = entries.len();

    if len == 0 {
        let empty = Line::from(Span::styled(
            "No user-protected processes. Enter a path below and press Enter to add.",
            Style::default().fg(theme.fg_muted).bg(theme.bg_overlay),
        ));
        f.render_widget(Paragraph::new(Text::from(empty)).style(base), area);
        return;
    }

    // Scroll offset keeps the selected entry visible (ports.rs pattern).
    let offset = if len > viewport {
        app.whitelist_selected.saturating_sub(viewport - 1)
    } else {
        0
    };
    let end = (offset + viewport).min(len);

    let is_list_focus = app.whitelist_focus == WhitelistFocus::List;
    let mut lines: Vec<Line> = Vec::with_capacity(end - offset);
    for (i, path) in entries.iter().enumerate().take(end).skip(offset) {
        let text = truncate(&format!("\u{2192} {}", path), area.width as usize);
        let style = if i == app.whitelist_selected {
            if is_list_focus {
                Style::default()
                    .fg(theme.fg_default)
                    .bg(theme.bg_selection)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(theme.fg_default).bg(theme.bg_selection)
            }
        } else {
            Style::default().fg(theme.fg_default).bg(theme.bg_overlay)
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    f.render_widget(Paragraph::new(Text::from(lines)).style(base), area);

    // Scrollbar (ports.rs render_scrollbar pattern) when overflowing.
    if len > viewport {
        render_scrollbar(f, area, offset as u16, len as u16, viewport as u16, theme);
    }
}

/// Row 17: "Path: >_" input with block cursor (search.rs pattern verbatim).
fn render_input_row(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let base = Style::default().bg(theme.bg_overlay);
    let prompt_style = Style::default()
        .fg(theme.accent_primary)
        .add_modifier(Modifier::BOLD)
        .bg(theme.bg_overlay);
    let text_style = Style::default().fg(theme.fg_default).bg(theme.bg_overlay);
    let muted_style = Style::default().fg(theme.fg_muted).bg(theme.bg_overlay);

    let mut spans: Vec<Span> = vec![Span::styled("Path: >", prompt_style)];
    let input = &app.whitelist_input;
    let cursor = app.whitelist_input_cursor;

    if input.is_empty() {
        // Placeholder when empty (search.rs pattern).
        spans.push(Span::styled("\u{2588}", text_style));
        spans.push(Span::styled(" type a path... use absolute paths", muted_style));
    } else {
        for (i, ch) in input.char_indices() {
            if i == cursor {
                spans.push(Span::styled("\u{2588}", text_style));
            }
            spans.push(Span::styled(ch.to_string(), text_style));
        }
        // Cursor at end of input.
        if cursor >= input.len() {
            spans.push(Span::styled("\u{2588}", text_style));
        }
    }

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(Text::from(line)).style(base), area);
}

/// Vertical scrollbar on the rightmost column of the user-list area
/// (ports.rs render_scrollbar pattern): `│` track, `█` thumb.
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
        y: area.y,
        width: 1,
        height: area.height,
    };

    let track_height = scroll_area.height as usize;
    if track_height == 0 {
        return;
    }

    let thumb_size = (((viewport as f64 / total as f64) * track_height as f64)
        .ceil() as usize)
        .max(1);
    // total > viewport guaranteed by the caller (only rendered on overflow).
    let thumb_pos = ((scroll_offset as f64 / (total - viewport) as f64)
        * (track_height - thumb_size) as f64) as usize;

    let mut lines: Vec<Line> = Vec::with_capacity(track_height);
    for i in 0..track_height {
        let ch = if i >= thumb_pos && i < thumb_pos + thumb_size {
            "\u{2588}"
        } else {
            "\u{2502}"
        };
        let style = Style::default().fg(theme.fg_muted).bg(theme.bg_overlay);
        lines.push(Line::styled(ch, style));
    }
    f.render_widget(Paragraph::new(Text::from(lines)), scroll_area);
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
