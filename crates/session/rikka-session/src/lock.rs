use rikka_seam::{Event, Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.session-lock", Version::new(1, 2));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATE: &str = "state";
pub const LOCK_REQUESTED: &str = "wanted";
pub const CHANGED: &str = "changed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockState {
    #[default]
    Open,
    Locking,
    Locked,
    Sealed,
    Abandoned,
}

impl LockState {
    #[must_use]
    pub fn needs_locker(self) -> bool {
        matches!(self, LockState::Sealed | LockState::Abandoned)
    }
}

#[must_use]
pub fn state(seam: &SeamClient) -> Reply {
    PROTOCOL.call(seam, STATE)
}

#[must_use]
pub fn is_lock_request(event: &Event) -> bool {
    PROTOCOL.decode_event::<()>(event, LOCK_REQUESTED).is_some()
}

#[must_use]
pub fn changed(event: &Event) -> Option<LockState> {
    PROTOCOL.decode_event(event, CHANGED)
}
