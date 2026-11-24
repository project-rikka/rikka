use futures_util::Stream;
use rikka_seam::{SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.pointer", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATE: &str = "state";
pub const CHANGED: &str = "changed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerMode {
    #[default]
    Absolute,
    Relative,
}

pub fn modes(seam: &SeamClient) -> impl Stream<Item = PointerMode> + Send + 'static {
    PROTOCOL.states(seam, CHANGED, STATE)
}
