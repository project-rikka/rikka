use rikka_hifi_core::{Capabilities, DeviceId, DsdSupport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: DeviceId,
    pub card: u32,
    pub name: String,
    pub pcm: String,
    pub capabilities: Option<Capabilities>,
    pub mixer: Option<String>,
}

impl Device {
    #[must_use]
    pub fn dsd(&self) -> DsdSupport {
        self.capabilities
            .as_ref()
            .map(Capabilities::dsd)
            .unwrap_or_default()
    }
}
