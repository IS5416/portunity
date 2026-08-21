//! Port-related data models.

/// A network port with its state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Port {
    pub number: u16,
    pub protocol: Protocol,
    pub state: PortState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
    Tcp6,
    Udp6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PortState {
    Listen,
    Established,
    CloseWait,
    TimeWait,
    SynSent,
    SynReceived,
    FinWait1,
    FinWait2,
    LastAck,
    Closing,
    Unknown,
}
