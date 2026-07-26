//! Component trait and tab panel implementations.
//!
//! Each tab implements the `Component` trait, receiving a render area
//! and read-only access to application state.

use ratatui::layout::Rect;
use ratatui::Frame;

pub mod ports;

pub use ports::PortsComponent;

/// Trait for renderable TUI components.
///
/// Components receive a reference to the app state, the frame, a render area,
/// and the current theme. They are stateless — all state lives in `App`.
pub trait Component {
    fn render(
        &self,
        app: &crate::app::App,
        f: &mut Frame,
        area: Rect,
        theme: &crate::theme::Theme,
    );
}
