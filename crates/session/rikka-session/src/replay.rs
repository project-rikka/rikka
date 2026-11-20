use std::path::PathBuf;

use futures_util::Stream;
use rikka_seam::{Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.replay", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATUS: &str = "held";
pub const SAVE: &str = "save";
pub const DRAFT: &str = "draft";
pub const KEEP: &str = "keep";
pub const DISCARD: &str = "discard";

pub const CHANGED: &str = "changed";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayStatus {
    pub running: bool,
    pub subjects: Vec<BufferedSubject>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BufferedSubject {
    pub id: String,
    pub kind: Kind,
    pub seconds: f64,
    pub ended: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Kind {
    Display { name: String, description: String },
    Window { app_id: String, title: String },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Save {
    pub subject: Option<String>,
    pub seconds: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub file: PathBuf,
    pub seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftRequest {
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub file: PathBuf,
    pub seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keep {
    pub draft: PathBuf,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug)]
pub struct ReplayClient {
    seam: SeamClient,
}

impl ReplayClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn status(&self) -> Reply {
        PROTOCOL.call(&self.seam, STATUS)
    }

    pub fn save(&self, save: &Save) -> Reply {
        PROTOCOL.call_with(&self.seam, SAVE, save)
    }

    pub fn draft(&self, subject: impl Into<String>) -> Reply {
        PROTOCOL.call_with(
            &self.seam,
            DRAFT,
            &DraftRequest {
                subject: subject.into(),
            },
        )
    }

    pub fn keep(&self, keep: &Keep) -> Reply {
        PROTOCOL.call_with(&self.seam, KEEP, keep)
    }

    pub fn discard(&self, draft: PathBuf) -> Reply {
        PROTOCOL.call_with(&self.seam, DISCARD, &draft)
    }
}

pub fn statuses(seam: &SeamClient) -> impl Stream<Item = ReplayStatus> + Send + 'static {
    PROTOCOL.states(seam, CHANGED, STATUS)
}
