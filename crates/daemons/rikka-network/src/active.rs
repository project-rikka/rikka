use crate::access_point::ApId;
use crate::device::DeviceId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActiveId(pub(crate) String);

impl ActiveId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveState {
    #[default]
    Unknown,
    Activating,
    Activated,
    Deactivating,
    Deactivated,
}

impl ActiveState {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Activating,
            2 => Self::Activated,
            3 => Self::Deactivating,
            4 => Self::Deactivated,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Activating | Self::Deactivating)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Active {
    pub id: ActiveId,
    pub uuid: String,
    pub title: String,
    pub state: ActiveState,
    pub devices: Vec<DeviceId>,
    pub default4: bool,
    pub default6: bool,
    pub vpn: bool,
    pub access_point: Option<ApId>,
}
