use rikka_seam::{Event, Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.toplevels", Version::new(2, 2));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATE: &str = "state";
pub const FOCUS: &str = "focus";
pub const CLOSE: &str = "close";
pub const MOVE: &str = "move";
pub const MINIMIZE: &str = "minimize";
pub const MAXIMIZE: &str = "maximize";
pub const UNMAXIMIZE: &str = "unmaximize";

pub const CHANGED: &str = "changed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    #[must_use]
    pub fn area(&self) -> i64 {
        i64::from(self.width.max(0)) * i64::from(self.height.max(0))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toplevel {
    pub id: String,
    pub app_id: String,
    pub title: String,
    pub output: String,
    pub workspace: usize,
    pub focused: bool,
    pub bounds: Bounds,
    pub on_screen: bool,
    #[serde(default)]
    pub minimized: bool,
    #[serde(default)]
    pub maximized: bool,
    #[serde(default)]
    pub tiled: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub toplevels: Vec<Toplevel>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToplevelRef {
    pub id: String,
}

impl ToplevelRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveRequest {
    pub id: String,
    pub workspace: usize,
}

#[derive(Clone, Debug)]
pub struct ToplevelsClient {
    seam: SeamClient,
}

impl ToplevelsClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn state(&self) -> Reply {
        PROTOCOL.call(&self.seam, STATE)
    }

    pub fn focus(&self, id: impl Into<String>) -> Reply {
        self.ask(FOCUS, id)
    }

    pub fn close(&self, id: impl Into<String>) -> Reply {
        self.ask(CLOSE, id)
    }

    pub fn minimize(&self, id: impl Into<String>) -> Reply {
        self.ask(MINIMIZE, id)
    }

    pub fn maximize(&self, id: impl Into<String>) -> Reply {
        self.ask(MAXIMIZE, id)
    }

    pub fn unmaximize(&self, id: impl Into<String>) -> Reply {
        self.ask(UNMAXIMIZE, id)
    }

    pub fn move_to_workspace(&self, id: impl Into<String>, workspace: usize) -> Reply {
        PROTOCOL.call_with(
            &self.seam,
            MOVE,
            &MoveRequest {
                id: id.into(),
                workspace,
            },
        )
    }

    pub fn notify_close(&self, id: impl Into<String>) {
        drop(self.close(id));
    }

    fn ask(&self, method: &'static str, id: impl Into<String>) -> Reply {
        PROTOCOL.call_with(&self.seam, method, &ToplevelRef::new(id))
    }
}

#[must_use]
pub fn changed(event: &Event) -> Option<State> {
    PROTOCOL.decode_event(event, CHANGED)
}

pub fn states(seam: &SeamClient) -> impl futures_util::Stream<Item = State> + Send + 'static {
    PROTOCOL.states(seam, CHANGED, STATE)
}
