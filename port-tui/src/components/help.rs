//! Help overlay component ('?' key, UI-SPEC §Help Overlay + Keyboard
//! Contract).
//!
//! Full key reference covering every keyboard capability: L0 universal,
//! navigation, L2 actions (including the footer-dropped `s` and `w` — this
//! overlay is their canonical reference), L2-confirm, and L3 power. Also
//! surfaces two user-facing notes: command lines are display-only (prohibition
//! P4) and protection matching is name-based (threat T-02-06).
//!
//! Stateless Clear-over covering the content area, rendered below the confirm
//! dialog in the overlay stack (confirm stays topmost). Hotkey chars render
//! in accent_primary, section labels in fg_muted — no new theme slots.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Help overlay component — stateless renderer.
pub struct HelpComponent;

impl Component for HelpComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        let _ = app; // stateless — no App state is read

        // Soft-overlay pattern: clear the surface first.
        f.render_widget(Clear, area);

        let base = Style::default().bg(theme.bg_overlay);
        let muted = Style::default().fg(theme.fg_muted).bg(theme.bg_overlay);
        let key = Style::default().fg(theme.accent_primary).bg(theme.bg_overlay);

        // Bordered block (overview.rs placeholder style) with Bold title.
        let block = Block::default()
            .borders(Borders::ALL)
            .style(base)
            .title(Line::from(Span::styled(
                "Help",
                Style::default()
                    .fg(theme.fg_emphasis)
                    .add_modifier(Modifier::BOLD),
            )));
        let inner = block.inner(area);

        let lines = vec![
            section("Universal", muted),
            keys(
                &[
                    ("[1]-[5]", "Tabs"),
                    ("[Tab]/[Shift+Tab]", "Next/prev tab"),
                    ("[Esc]", "Close overlay"),
                ],
                &muted,
                &key,
            ),
            Line::from(""),
            section("Navigation", muted),
            keys(
                &[("[j]/[k]", "Move"), ("[g]/[G]", "Top/bottom")],
                &muted,
                &key,
            ),
            Line::from(""),
            section("Actions", muted),
            keys(
                &[
                    ("[d]", "Detail"),
                    ("[x]", "Kill"),
                    ("[w]", "Whitelist"),
                    ("[s]", "Sort"),
                    ("[r]", "Refresh"),
                ],
                &muted,
                &key,
            ),
            keys(
                &[
                    ("[/]", "Search"),
                    ("[f]", "Filter"),
                    ("[a]", "Elevate (when not admin)"),
                ],
                &muted,
                &key,
            ),
            Line::from(""),
            section("Kill confirmation", muted),
            keys(
                &[("[y]", "Confirm kill"), ("[n]", "Cancel")],
                &muted,
                &key,
            ),
            Line::from(""),
            section("Power", muted),
            keys(&[("[q]", "Quit")], &muted, &key),
            Line::from(""),
            Line::from(Span::styled(
                "Command lines shown in the detail panel may contain secrets — they are \
                 displayed locally only and never persisted or exported.",
                muted,
            )),
            Line::from(Span::styled(
                "Protection matching is name-based — all instances of a built-in name \
                 (e.g. svchost.exe) are protected.",
                muted,
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Esc]", key),
                Span::styled(" Close", muted),
            ]),
        ];

        let paragraph = Paragraph::new(Text::from(lines))
            .style(base)
            .alignment(Alignment::Left);
        f.render_widget(paragraph, inner);
        f.render_widget(block, area);
    }
}

/// A fg_muted section label line.
fn section<'a>(label: &'a str, muted: Style) -> Line<'a> {
    Line::from(Span::styled(label, muted))
}

/// A key-reference line: hotkey chars in accent_primary, descriptions muted,
/// three-space separators between pairs.
fn keys<'a>(
    pairs: &[(&'a str, &'a str)],
    muted: &Style,
    key: &Style,
) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    for (i, (hotkey, desc)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", *muted));
        }
        spans.push(Span::styled(*hotkey, *key));
        spans.push(Span::styled(format!(" {}", desc), *muted));
    }
    Line::from(spans)
}

