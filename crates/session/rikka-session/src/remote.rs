use std::fmt;

use futures_util::Stream;
use rikka_seam::{Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.remote", Version::new(2, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATUS: &str = "status";
pub const DISCONNECT: &str = "disconnect";

pub const CHANGED: &str = "changed";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoteStatus {
    pub serving: Option<Serving>,
    pub viewer: Option<Viewer>,
    pub problem: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Serving {
    pub port: u16,
    pub addresses: Vec<Address>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub address: String,
    pub reach: Reach,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    Local,
    Tailnet,
    Network,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewer {
    pub address: String,
    pub device: String,
    pub since: i64,
    pub encoding: Option<Encoding>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Encoding {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub bitrate: u64,
    pub encoder: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    H264,
    H265,
}

impl fmt::Display for Codec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Codec::H264 => "H.264",
            Codec::H265 => "H.265",
        })
    }
}

#[derive(Clone, Debug)]
pub struct RemoteClient {
    seam: SeamClient,
}

impl RemoteClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn status(&self) -> Reply {
        PROTOCOL.call(&self.seam, STATUS)
    }

    pub fn disconnect(&self) -> Reply {
        PROTOCOL.call(&self.seam, DISCONNECT)
    }
}

pub fn statuses(seam: &SeamClient) -> impl Stream<Item = RemoteStatus> + Send + 'static {
    PROTOCOL.states(seam, CHANGED, STATUS)
}
