use crate::adapter::AdapterId;
use crate::device::DeviceId;

#[derive(Clone, Debug)]
pub enum Request {
    Power(AdapterId, bool),
    Discoverable(AdapterId, bool),
    Pairable(AdapterId, bool),
    Rename(AdapterId, String),
    Scan(AdapterId, bool),
    Pair(DeviceId),
    Cancel(DeviceId),
    Connect(DeviceId),
    Disconnect(DeviceId),
    Trust(DeviceId, bool),
    Block(DeviceId, bool),
    Label(DeviceId, String),
    Forget(DeviceId),
}

impl Request {
    #[must_use]
    pub fn action(&self) -> Action {
        match self {
            Self::Power(..) => Action::Power,
            Self::Discoverable(..) => Action::Discoverable,
            Self::Pairable(..) => Action::Pairable,
            Self::Rename(..) => Action::Rename,
            Self::Scan(..) => Action::Scan,
            Self::Pair(_) => Action::Pair,
            Self::Cancel(_) => Action::Cancel,
            Self::Connect(_) => Action::Connect,
            Self::Disconnect(_) => Action::Disconnect,
            Self::Trust(..) => Action::Trust,
            Self::Block(..) => Action::Block,
            Self::Label(..) => Action::Label,
            Self::Forget(_) => Action::Forget,
        }
    }

    #[must_use]
    pub fn device(&self) -> Option<&DeviceId> {
        match self {
            Self::Pair(id)
            | Self::Cancel(id)
            | Self::Connect(id)
            | Self::Disconnect(id)
            | Self::Trust(id, _)
            | Self::Block(id, _)
            | Self::Label(id, _)
            | Self::Forget(id) => Some(id),
            Self::Power(..)
            | Self::Discoverable(..)
            | Self::Pairable(..)
            | Self::Rename(..)
            | Self::Scan(..) => None,
        }
    }

    pub(crate) fn is_slow(&self) -> bool {
        matches!(self, Self::Pair(_) | Self::Connect(_) | Self::Disconnect(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Power,
    Discoverable,
    Pairable,
    Rename,
    Scan,
    Pair,
    Cancel,
    Connect,
    Disconnect,
    Trust,
    Block,
    Label,
    Forget,
}
