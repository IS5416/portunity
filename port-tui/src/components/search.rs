//! Search bar overlay component.
//!
//! Renders a fuzzy-search input bar triggered by the '/' key.
//! The search bar appears as an overlay at the top of the content area,
//! using `Clear` to overwrite any port table rows behind it.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::components::Component;
use crate::theme::Theme;

/// Search bar component — stateless renderer.
pub struct SearchComponent;

impl Component for SearchComponent {
    fn render(&self, app: &App, f: &mut Frame, area: Rect, theme: &Theme) {
        // 3-row overlay at the top of the content area
        let search_area = Layout::vertical([
            Constraint::Length(1), // spacing
            Constraint::Length(1), // input row
            Constraint::Length(1), // help hint
        ])
        .split(area);

        // Row 0: spacing (no render)
        // Row 1: search input
        let input_area = search_area[1];

        // Clear background behind the search bar
        f.render_widget(Clear, input_area);

        let query = &app.search_query;
        let cursor_pos = app.search_cursor_pos;

        // Build styled prompt: "/> " + query text + cursor
        let prompt_style = Style::default()
            .fg(theme.accent_primary)
            .add_modifier(Modifier::BOLD)
            .bg(theme.bg_overlay);
        let text_style = Style::default()
            .fg(theme.fg_default)
            .bg(theme.bg_overlay);
        let muted_style = Style::default()
            .fg(theme.fg_muted)
            .bg(theme.bg_overlay);

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled("/> ", prompt_style));

        if query.is_empty() {
            // Placeholder text when empty
            spans.push(Span::styled("type to search...", muted_style));
        } else {
            // Render query with cursor
            for (i, ch) in query.char_indices() {
                if i == cursor_pos {
                    spans.push(Span::styled(
                        "\u{2588}", // block cursor
                        text_style,
                    ));
                }
                spans.push(Span::styled(ch.to_string(), text_style));
            }
            // Cursor at end of query
            if cursor_pos >= query.len() {
                spans.push(Span::styled("\u{2588}", text_style));
            }
        }

        let input_line = Line::from(spans);
        let paragraph = Paragraph::new(Text::from(input_line))
            .style(Style::default().bg(theme.bg_overlay));
        f.render_widget(paragraph, input_area);

        // Row 2: help hint
        let help_area = search_area[2];
        f.render_widget(Clear, help_area);

        let help_text = Span::styled(
            "[Esc]Cancel [Enter]Confirm  —  fuzzy search across all fields",
            muted_style,
        );
        let help = Paragraph::new(Text::from(Line::from(help_text)))
            .alignment(Alignment::Center)
            .style(Style::default().bg(theme.bg_overlay));
        f.render_widget(help, help_area);
    }
}
