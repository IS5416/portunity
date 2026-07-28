//! Traffic tab placeholder — real-time network traffic monitoring.
//!
//! Content deferred to Phase 3 (TRAF-01 through TRAF-03).
//! Renders a centered "Coming later" message with navigation hint per UI-SPEC.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Traffic tab placeholder component.
pub struct TrafficTabComponent;

impl Component for TrafficTabComponent {
    fn render(&self, _app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.fg_muted))
            .style(Style::default().bg(theme.bg_base));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let dim = Style::default()
            .fg(theme.fg_muted)
            .bg(theme.bg_base)
            .add_modifier(Modifier::DIM);

        let text = Text::from(vec![
            Line::from(""),
            Line::from(Span::styled("Coming later", dim)),
            Line::from(""),
            Line::from(Span::styled(
                "This tab will be available in a future",
                dim,
            )),
            Line::from(Span::styled(
                "phase. Press 1 or 2 to view active tabs.",
                dim,
            )),
        ]);

        let text_height = 5u16;
        let v_padding = inner.height.saturating_sub(text_height) / 2;
        let centered_area = Rect {
            y: inner.y + v_padding,
            height: text_height,
            ..inner
        };

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().bg(theme.bg_base));
        f.render_widget(paragraph, centered_area);
    }
}
