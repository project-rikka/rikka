use std::path::PathBuf;

use futures_util::Stream;
use rikka_seam::{Event, Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.wallpaper", Version::new(3, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const LIST: &str = "list";
pub const REFRESH: &str = "refresh";
pub const SHOW: &str = "show";
pub const SHOWING: &str = "showing";
pub const RESERVE: &str = "reserve";
pub const RESERVED: &str = "reserved";

pub const LISTED: &str = "listed";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperEntry {
    pub id: String,
    pub name: String,
    pub preview: Preview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Preview {
    Still { thumbnail: Thumbnail },
    Motion { file: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Thumbnail {
    Made { file: PathBuf },
    Making,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayedWallpaper {
    pub id: String,
    pub file: PathBuf,
}

#[derive(Clone, Debug)]
pub struct WallpaperClient {
    seam: SeamClient,
}

impl WallpaperClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn list(&self) -> Reply {
        PROTOCOL.call(&self.seam, LIST)
    }

    pub fn refresh(&self) {
        PROTOCOL.notify(&self.seam, REFRESH);
    }

    pub fn show(&self, id: Option<String>) {
        PROTOCOL.notify_with(&self.seam, SHOW, &id);
    }

    pub fn reserve(&self, strip: f32) {
        PROTOCOL.notify_with(&self.seam, RESERVE, &strip);
    }

    pub fn showing(&self) -> Reply {
        PROTOCOL.call(&self.seam, SHOWING)
    }
}

#[must_use]
pub fn listed(event: &Event) -> Option<Vec<WallpaperEntry>> {
    PROTOCOL.decode_event(event, LISTED)
}

pub fn listings(seam: &SeamClient) -> impl Stream<Item = Vec<WallpaperEntry>> + Send + 'static {
    PROTOCOL.states(seam, LISTED, LIST)
}

pub fn displayed(
    seam: &SeamClient,
) -> impl Stream<Item = Option<DisplayedWallpaper>> + Send + 'static {
    PROTOCOL.states(seam, SHOWING, SHOWING)
}

pub fn reserves(seam: &SeamClient) -> impl Stream<Item = f32> + Send + 'static {
    PROTOCOL.states(seam, RESERVED, RESERVED)
}
