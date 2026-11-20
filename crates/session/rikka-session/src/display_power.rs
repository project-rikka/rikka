use rikka_seam::{Event, Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.display-power", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATE: &str = "state";
pub const SLEEP: &str = "sleep";
pub const CHANGED: &str = "changed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayPower {
    #[default]
    Awake,
    Asleep,
}

#[must_use]
pub fn state(seam: &SeamClient) -> Reply {
    PROTOCOL.call(seam, STATE)
}

pub fn sleep(seam: &SeamClient) {
    PROTOCOL.notify(seam, SLEEP);
}

#[must_use]
pub fn changed(event: &Event) -> Option<DisplayPower> {
    PROTOCOL.decode_event(event, CHANGED)
}
