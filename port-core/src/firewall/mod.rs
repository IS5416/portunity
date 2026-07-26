//! Windows Firewall rule management.

pub trait FirewallManager {
    fn list_rules(&self) -> crate::Result<Vec<crate::models::FirewallRule>>;
    fn add_rule(&self, rule: &crate::models::FirewallRule) -> crate::Result<()>;
    fn remove_rule(&self, name: &str) -> crate::Result<()>;
    fn toggle_rule(&self, name: &str, enabled: bool) -> crate::Result<()>;
}
