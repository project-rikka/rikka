use std::time::Duration;

use rikka_seam::{Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.agent", Version::new(1, 1));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const SEAT: &str = "rikka-agent";

pub const PLACEMENT: &str = "where";
pub const SHOW: &str = "show";
pub const WAIT_IDLE: &str = "wait-idle";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentPlacement {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub x: f64,
    pub y: f64,
    #[serde(default, rename = "shown")]
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitIdleRequest {
    pub quiet_ms: u64,
    pub timeout_ms: u64,
}

impl WaitIdleRequest {
    #[must_use]
    pub fn new(quiet: Duration, timeout: Duration) -> WaitIdleRequest {
        WaitIdleRequest {
            quiet_ms: saturating_millis(quiet),
            timeout_ms: saturating_millis(timeout),
        }
    }

    #[must_use]
    pub fn quiet(self) -> Duration {
        Duration::from_millis(self.quiet_ms)
    }

    #[must_use]
    pub fn timeout(self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Idle {
    Quiescent,
    TimedOut,
}

#[derive(Clone, Debug)]
pub struct AgentClient {
    seam: SeamClient,
}

impl AgentClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> AgentClient {
        AgentClient { seam }
    }

    pub fn placement(&self) -> Reply {
        PROTOCOL.call(&self.seam, PLACEMENT)
    }

    pub fn show(&self, visible: bool) -> Reply {
        PROTOCOL.call_with(&self.seam, SHOW, &visible)
    }

    pub fn wait_idle(&self, quiet: Duration, timeout: Duration) -> Reply {
        PROTOCOL.call_with(&self.seam, WAIT_IDLE, &WaitIdleRequest::new(quiet, timeout))
    }
}

fn saturating_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
