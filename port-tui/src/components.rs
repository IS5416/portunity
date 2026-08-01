//! Component trait and tab panel implementations.
//!
//! Each tab implements the `Component` trait, receiving a render area
//! and read-only access to application state.

use ratatui::layout::Rect;
use ratatui::Frame;

pub mod ports;
pub mod search;
pub mod filter_panel;
pub mod overview;
pub mod history;
pub mod traffic;
pub mod firewall;
pub mod kill_confirm;

pub use ports::PortsComponent;
pub use search::SearchComponent;
pub use filter_panel::FilterPanelComponent;
pub use overview::OverviewComponent;
pub use history::HistoryTabComponent;
pub use traffic::TrafficTabComponent;
pub use firewall::FirewallTabComponent;
pub use kill_confirm::KillConfirmComponent;

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
