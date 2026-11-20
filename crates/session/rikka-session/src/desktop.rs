use std::path::PathBuf;

use rikka_seam::{Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.desktop", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATUS: &str = "status";
pub const START: &str = "start";
pub const STOP: &str = "stop";
pub const RESTART: &str = "restart";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub program: PathBuf,
    #[serde(default)]
    pub pid: Option<u32>,
}

impl Status {
    #[must_use]
    pub fn running(&self) -> bool {
        self.pid.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct DesktopClient {
    seam: SeamClient,
}

impl DesktopClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn status(&self) -> Reply {
        self.call(STATUS)
    }

    pub fn start(&self) -> Reply {
        self.call(START)
    }

    pub fn stop(&self) -> Reply {
        self.call(STOP)
    }

    pub fn restart(&self) -> Reply {
        self.call(RESTART)
    }

    fn call(&self, method: &'static str) -> Reply {
        PROTOCOL.call(&self.seam, method)
    }
}
