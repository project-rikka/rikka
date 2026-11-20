use std::path::PathBuf;

use crate::access_point::ApId;
use crate::active::ActiveId;
use crate::device::DeviceId;
use crate::hotspot::Hotspot;
use crate::profile::{Eap, Profile};
use crate::security::Security;
use crate::ssid::Ssid;

#[derive(Clone, Debug)]
pub enum JoinTarget {
    Visible(ApId),
    Hidden(Ssid),
}

#[derive(Clone, Debug)]
pub struct Join {
    pub device: DeviceId,
    pub network: JoinTarget,
    pub security: Security,
    pub passphrase: String,
    pub account: Option<Box<Eap>>,
    pub shared: bool,
}

#[derive(Clone, Debug)]
pub enum Request {
    Scan(DeviceId),
    Radio(bool),
    Networking(bool),
    Join(Box<Join>),
    Activate {
        uuid: String,
        device: Option<DeviceId>,
    },
    Deactivate(ActiveId),
    Disconnect(DeviceId),
    Forget(String),
    Save(Box<Profile>),
    Reveal(String),
    Hotspot(Box<Hotspot>),
    Import(PathBuf),
    Recheck,
}

impl Request {
    #[must_use]
    pub fn action(&self) -> Action {
        match self {
            Self::Scan(_) => Action::Scan,
            Self::Radio(_) => Action::Radio,
            Self::Networking(_) => Action::Networking,
            Self::Join(_) => Action::Join,
            Self::Activate { .. } => Action::Activate,
            Self::Deactivate(_) => Action::Deactivate,
            Self::Disconnect(_) => Action::Disconnect,
            Self::Forget(_) => Action::Forget,
            Self::Save(_) => Action::Save,
            Self::Reveal(_) => Action::Reveal,
            Self::Hotspot(_) => Action::Hotspot,
            Self::Import(_) => Action::Import,
            Self::Recheck => Action::Recheck,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Scan,
    Radio,
    Networking,
    Join,
    Activate,
    Deactivate,
    Disconnect,
    Forget,
    Save,
    Reveal,
    Hotspot,
    Import,
    Recheck,
}

impl Action {
    pub(crate) fn concerns_join(self) -> bool {
        matches!(self, Action::Join | Action::Activate | Action::Save)
    }
}
