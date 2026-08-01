//! Detail panel overlay component (D-05..D-08, PROC-06, UI-SPEC §Detail
//! Panel Internal Layout).
//!
//! 12-row top-anchored Clear-over: `{name} — PID {pid}` title with protection
//! badge, Status, Owning port, Executable path, Command line, Start time,
//! Parent PID, Signature, Protection, Reason, kill hint, bottom border.
//! Stateless renderer — all state lives in `App` (detail_active / detail_pid
//! / detail_data / detail_loading / detail_exited / signature_cache).
//!
//! Non-modal: the table stays live below; j/k/up/down/r/s/g/G//f pass through
//! to the table (main.rs) — selection change refreshes the panel (D-06).
//! Keyboard: d/Esc close (main.rs).
//!
//! All copy comes verbatim from the UI-SPEC Copywriting Contract.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Label column width — "Executable path:" (16 chars) + one space (UI-SPEC).
const LABEL_COL: usize = 17;

/// Detail panel component — stateless renderer.
pub struct DetailPanelComponent;

impl Component for DetailPanelComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        // Clear the overlay area first (soft-overlay pattern)
        f.render_widget(Clear, area);

        let base = Style::default().bg(theme.bg_overlay);
        let muted = Style::default().fg(theme.fg_muted).bg(theme.bg_overlay);
        let dim = Style::default()
            .fg(theme.fg_muted)
            .add_modifier(Modifier::DIM)
            .bg(theme.bg_overlay);

        // Data sources: fetched detail data (falling back to the row's
        // scan-time process info) + the selected row for port/protocol.
        let conn = app.selected_connection();
        let display = app
            .detail_data
            .as_ref()
            .filter(|d| Some(d.pid) == app.detail_pid)
            .or_else(|| conn.map(|c| &c.process));

        let name = display
            .filter(|p| !p.name.is_empty())
            .or(conn.map(|c| &c.process))
            .map(|p| p.name.as_str())
            .unwrap_or("");
        let pid = app
            .detail_pid
            .or_else(|| conn.map(|c| c.process.pid))
            .unwrap_or(0);
        let loading = app.detail_loading && app.detail_data.is_none();

        // No-selection state (UI-SPEC Detail Panel States)
        if conn.is_none() && !loading {
            let rows = body_rows(area);
            let line = Line::from(Span::styled(
                "No port selected — use j/k to move through the list",
                Style::default()
                    .fg(theme.fg_muted)
                    .bg(theme.bg_overlay),
            ));
            f.render_widget(Paragraph::new(Text::from(line)).style(base), rows[0]);
            render_bottom_border(f, area, theme);
            return;
        }

        // ── Row 0: title "{name} — PID {pid}" + protection badge + [Esc] ──
        let critical = display.map(|p| p.is_system_critical).unwrap_or(false);
        let user_protected = display.map(|p| p.user_protected).unwrap_or(false);

        let badge: &str = if critical {
            "[PROTECTED]"
        } else if user_protected {
            "[CONFIRM]"
        } else {
            ""
        };
        let badge_style = if critical {
            theme.status_error
        } else {
            theme.status_warning
        };

        // Name budget: keep badge + [Esc] visible at the 80-col minimum.
        let chrome = 7u16 // " — PID "
            + pid.to_string().len() as u16
            + badge.len() as u16
            + 6; // " [Esc]"
        let name_budget = area.width.saturating_sub(chrome) as usize;
        let display_name = truncate(name, name_budget);

        let title_style = Style::default()
            .fg(theme.fg_emphasis)
            .add_modifier(Modifier::BOLD)
            .bg(theme.bg_overlay);
        let mut title_spans = vec![
            Span::styled(
                format!("{} — PID {}", display_name, pid),
                if app.detail_exited {
                    // UI-SPEC Typography: strikethrough (SGR 9) when the
                    // process exited while the panel is open.
                    title_style.add_modifier(Modifier::CROSSED_OUT)
                } else {
                    title_style
                },
            ),
            Span::styled(" ", base),
        ];
        if !badge.is_empty() {
            title_spans.push(Span::styled(
                badge,
                Style::default()
                    .fg(badge_style)
                    .add_modifier(Modifier::BOLD)
                    .bg(theme.bg_overlay),
            ));
            title_spans.push(Span::styled(" ", base));
        }
        title_spans.push(Span::styled("[Esc]", muted));

        let rows = body_rows(area);
        f.render_widget(
            Paragraph::new(Text::from(Line::from(title_spans))).style(base),
            rows[0],
        );

        let label_style = muted;
        let value_style = Style::default().fg(theme.fg_default).bg(theme.bg_overlay);
        let value_width = (area.width as usize).saturating_sub(LABEL_COL);

        // Loading state: name/PID show immediately; field values render
        // "Loading details…" (UI-SPEC Detail Panel States).
        let value_or_loading = |v: Option<String>| -> String {
            if loading {
                "Loading details…".to_string()
            } else {
                v.unwrap_or_else(|| "—".to_string())
            }
        };

        // Row 1: Status — Running | Exited (strikethrough state)
        let status = if app.detail_exited {
            Span::styled(
                "Exited",
                Style::default()
                    .fg(theme.fg_default)
                    .add_modifier(Modifier::CROSSED_OUT)
                    .bg(theme.bg_overlay),
            )
        } else {
            Span::styled(if loading { "Loading details…" } else { "Running" }, value_style)
        };
        render_row(f, rows[1], "Status:", status, label_style, base);

        // Row 2: Owning port — "{local_port} ({protocol})" from the row
        let port_text = conn
            .map(|c| format!("{} ({})", c.port.number, protocol_label(c.port.protocol)))
            .unwrap_or_else(|| "—".to_string());
        render_row(
            f,
            rows[2],
            "Owning port:",
            Span::styled(value_or_loading(Some(port_text)), value_style),
            label_style,
            base,
        );

        // Row 3: Executable path — right-segment-preserving truncation
        let path = display.and_then(|p| p.executable_path.clone());
        let path_text = value_or_loading(path);
        let path_display = truncate_path_tail(&path_text, value_width);
        render_row(
            f,
            rows[3],
            "Executable path:",
            Span::styled(path_display, value_style),
            label_style,
            base,
        );

        // Row 4: Command line — right-truncate U+2026
        let cmdline = display.and_then(|p| p.command_line.clone());
        let cmd_display = truncate(&value_or_loading(cmdline), value_width);
        render_row(
            f,
            rows[4],
            "Command line:",
            Span::styled(cmd_display, value_style),
            label_style,
            base,
        );

        // Row 5: Start time — chrono "%H:%M:%S %d-%b-%Y" (UI-SPEC row 5)
        let start = display.and_then(|p| p.start_time).map(|st| {
            chrono::DateTime::<chrono::Utc>::from(st)
                .format("%H:%M:%S %d-%b-%Y")
                .to_string()
        });
        render_row(
            f,
            rows[5],
            "Start time:",
            Span::styled(value_or_loading(start), value_style),
            label_style,
            base,
        );

        // Row 6: Parent PID
        let parent = display
            .and_then(|p| p.parent_pid)
            .map(|ppid| ppid.to_string());
        render_row(
            f,
            rows[6],
            "Parent PID:",
            Span::styled(value_or_loading(parent), value_style),
            label_style,
            base,
        );

        // Row 7: Signature — Verifying… | Signed | Unsigned | Unknown (D-07)
        let signature = match app.signature_cache.get(&pid) {
            None => ("Verifying…", muted),
            Some(Some(true)) => ("Signed", Style::default().fg(theme.status_success).bg(theme.bg_overlay)),
            Some(Some(false)) => ("Unsigned", Style::default().fg(theme.status_warning).bg(theme.bg_overlay)),
            Some(None) => ("Unknown", muted),
        };
        render_row(
            f,
            rows[7],
            "Signature:",
            Span::styled(
                if loading { "Verifying…" } else { signature.0 },
                signature.1,
            ),
            label_style,
            base,
        );

        // Row 8: Protection
        let protection = if critical {
            "System-critical (built-in whitelist)"
        } else if user_protected {
            "On your protection list"
        } else {
            "Not protected"
        };
        render_row(
            f,
            rows[8],
            "Protection:",
            Span::styled(
                if loading { "Loading details…" } else { protection },
                value_style,
            ),
            label_style,
            base,
        );

        // Row 9: Reason — only when protected (UI-SPEC copy verbatim)
        let reason = if critical {
            format!(
                "{} is a core Windows system process. Terminating it would crash or destabilize the system.",
                display_name
            )
        } else if user_protected {
            format!("{} is on your protection list.", display_name)
        } else {
            "—".to_string()
        };
        render_row(
            f,
            rows[9],
            "Reason:",
            Span::styled(
                if loading { "Loading details…" } else { &reason },
                if critical || user_protected { value_style } else { dim },
            ),
            label_style,
            base,
        );

        // Row 10: Hint — accent_secondary kill-action highlight ONLY
        let hint = if critical {
            "Cannot terminate — system-critical"
        } else if user_protected {
            "Press x to terminate — confirmation required"
        } else {
            "Press x to terminate"
        };
        let hint_style = Style::default()
            .fg(theme.accent_secondary)
            .bg(theme.bg_overlay);
        render_row(f, rows[10], "", Span::styled(hint, hint_style), label_style, base);

        // Row 11: bottom border (U+2500 run)
        render_bottom_border(f, area, theme);
    }
}

/// Split the overlay into the 12 fixed rows.
fn body_rows(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Length(1), // status
        Constraint::Length(1), // owning port
        Constraint::Length(1), // executable path
        Constraint::Length(1), // command line
        Constraint::Length(1), // start time
        Constraint::Length(1), // parent pid
        Constraint::Length(1), // signature
        Constraint::Length(1), // protection
        Constraint::Length(1), // reason
        Constraint::Length(1), // hint
        Constraint::Length(1), // bottom border
    ])
    .split(area)
}

/// Render one label + value row. Label column is fixed at 17 chars.
fn render_row(
    f: &mut Frame,
    area: Rect,
    label: &str,
    value: Span<'_>,
    label_style: Style,
    base: Style,
) {
    let padded_label = format!("{:<width$}", label, width = LABEL_COL);
    let line = Line::from(vec![
        Span::styled(padded_label, label_style),
        value,
    ]);
    f.render_widget(Paragraph::new(Text::from(line)).style(base), area);
}

/// Row 11 — bottom border line (U+2500 run).
fn render_bottom_border(f: &mut Frame, area: Rect, theme: &Theme) {
    let border = "\u{2500}".repeat(area.width as usize);
    let line = Line::from(Span::styled(
        border,
        Style::default().fg(theme.fg_muted).bg(theme.bg_overlay),
    ));
    let rows = body_rows(area);
    f.render_widget(Paragraph::new(Text::from(line)).style(Style::default().bg(theme.bg_overlay)), rows[11]);
}

/// Short protocol display label (mirrors ports.rs).
fn protocol_label(p: port_core::models::Protocol) -> &'static str {
    match p {
        port_core::models::Protocol::Tcp => "TCP",
        port_core::models::Protocol::Udp => "UDP",
        port_core::models::Protocol::Tcp6 => "TCP6",
        port_core::models::Protocol::Udp6 => "UDP6",
    }
}

/// Right-truncate a string to max_len chars, appending U+2026 if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    } else {
        s.to_string()
    }
}

/// Right-segment-preserving truncation for paths: keeps the tail
/// (`…\dir\name.exe`) — never wraps, never loses the file name (UI-SPEC
/// overflow: "Executable path …\nodejs\node.exe").
fn truncate_path_tail(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let keep = max_len.saturating_sub(1);
        let tail: String = s.chars().skip(s.chars().count().saturating_sub(keep)).collect();
        format!("\u{2026}{}", tail)
    } else {
        s.to_string()
    }
}
