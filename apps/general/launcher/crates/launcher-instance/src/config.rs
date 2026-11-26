use std::collections::BTreeMap;

use launcher_meta::GameTarget;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub fullscreen: bool,
}

impl Window {
    pub const DEFAULT: Window = Window {
        width: 1280,
        height: 720,
        fullscreen: false,
    };
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRecord {
    pub size: u64,
    pub modified: i128,
    pub sha512: String,
    #[serde(default)]
    pub identified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub incompatible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub created: i64,
    #[serde(default)]
    pub game: Option<GameTarget>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub jvm_arguments: Option<String>,
    #[serde(default)]
    pub window: Option<Window>,
    #[serde(default)]
    pub play_time: u64,
    #[serde(default)]
    pub last_played: Option<i64>,
    #[serde(default)]
    pub content: BTreeMap<String, ContentRecord>,
}

impl Config {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Config {
        Config {
            name: name.into(),
            created: jiff::Timestamp::now().as_second(),
            game: None,
            memory_mb: None,
            jvm_arguments: None,
            window: None,
            play_time: 0,
            last_played: None,
            content: BTreeMap::new(),
        }
    }

    pub fn add_play_time(&mut self, seconds: u64, ended: i64) {
        self.play_time += seconds;
        self.last_played = Some(ended);
    }
}
