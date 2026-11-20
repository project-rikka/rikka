use crate::adapter::AdapterId;
use crate::kind::DeviceKind;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub(crate) String);

impl DeviceId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: DeviceId,
    pub adapter: AdapterId,
    pub address: String,
    pub title: String,
    pub kind: DeviceKind,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub blocked: bool,
    pub legacy_pairing: bool,
    pub signal: Option<i16>,
    pub battery: Option<u8>,
    pub services: Vec<String>,
}

impl Device {
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.paired || self.trusted
    }
}
