use rikka_seam::{Event, Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.workspaces", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATE: &str = "state";
pub const SHOW: &str = "show";

pub const CHANGED: &str = "changed";

pub const COUNT: usize = 10;

#[must_use]
pub fn exists(number: usize) -> bool {
    (1..=COUNT).contains(&number)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub count: usize,
    pub displays: Vec<Displayed>,
}

impl State {
    #[must_use]
    pub fn display(&self, output: &str) -> Option<&Displayed> {
        self.displays
            .iter()
            .find(|displayed| displayed.output == output)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Displayed {
    pub output: String,
    pub showing: usize,
    pub holding: [bool; COUNT],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Show {
    #[serde(default)]
    pub output: Option<String>,
    pub number: usize,
}

#[derive(Clone, Debug)]
pub struct WorkspacesClient {
    seam: SeamClient,
}

impl WorkspacesClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn state(&self) -> Reply {
        PROTOCOL.call(&self.seam, STATE)
    }

    pub fn show(&self, output: Option<String>, number: usize) -> Reply {
        PROTOCOL.call_with(&self.seam, SHOW, &Show { output, number })
    }
}

#[must_use]
pub fn changed(event: &Event) -> Option<State> {
    PROTOCOL.decode_event(event, CHANGED)
}

pub fn states(seam: &SeamClient) -> impl futures_util::Stream<Item = State> + Send + 'static {
    PROTOCOL.states(seam, CHANGED, STATE)
}
