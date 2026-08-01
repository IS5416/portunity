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
pub mod detail_panel;
pub mod whitelist_overlay;
pub mod help;

pub use ports::PortsComponent;
pub use search::SearchComponent;
pub use filter_panel::FilterPanelComponent;
pub use overview::OverviewComponent;
pub use history::HistoryTabComponent;
pub use traffic::TrafficTabComponent;
pub use firewall::FirewallTabComponent;
pub use kill_confirm::KillConfirmComponent;
pub use detail_panel::DetailPanelComponent;
pub use whitelist_overlay::WhitelistOverlayComponent;
pub use help::HelpComponent;

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
