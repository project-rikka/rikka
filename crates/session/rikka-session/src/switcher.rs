use futures_util::Stream;
use rikka_seam::{Event, Reply, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.switcher", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const STATE: &str = "state";
pub const SELECT: &str = "select";
pub const CHOOSE: &str = "choose";
pub const DISMISS: &str = "dismiss";
pub const ARRANGE: &str = "arrange";

pub const CHANGED: &str = "changed";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switch {
    pub serial: u64,
    pub output: String,
    pub windows: Vec<Candidate>,
    pub selected: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub app_id: String,
    pub title: String,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pick {
    pub serial: u64,
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dismissal {
    pub serial: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub serial: u64,
    pub rows: Rows,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rows(pub Vec<Vec<f32>>);

impl Rows {
    #[must_use]
    pub fn above(&self, from: usize) -> Option<usize> {
        self.beside(from, -1)
    }

    #[must_use]
    pub fn below(&self, from: usize) -> Option<usize> {
        self.beside(from, 1)
    }

    fn beside(&self, from: usize, by: isize) -> Option<usize> {
        let (row, centre) = self.locate(from)?;
        let target_row = row.checked_add_signed(by)?;
        let first = self.0.iter().take(target_row).map(Vec::len).sum::<usize>();
        let nearest = self
            .0
            .get(target_row)?
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - centre).abs().total_cmp(&(*b - centre).abs()))?
            .0;
        Some(first + nearest)
    }

    fn locate(&self, index: usize) -> Option<(usize, f32)> {
        let mut first = 0;
        for (row, centres) in self.0.iter().enumerate() {
            if index < first + centres.len() {
                return centres.get(index - first).map(|centre| (row, *centre));
            }
            first += centres.len();
        }
        None
    }
}

#[derive(Clone, Debug)]
pub struct SwitcherClient {
    seam: SeamClient,
}

impl SwitcherClient {
    #[must_use]
    pub fn new(seam: SeamClient) -> Self {
        Self { seam }
    }

    pub fn state(&self) -> Reply {
        PROTOCOL.call(&self.seam, STATE)
    }

    pub fn select(&self, serial: u64, id: impl Into<String>) {
        let id = id.into();
        PROTOCOL.notify_with(&self.seam, SELECT, &Pick { serial, id });
    }

    pub fn choose(&self, serial: u64, id: impl Into<String>) {
        let id = id.into();
        PROTOCOL.notify_with(&self.seam, CHOOSE, &Pick { serial, id });
    }

    pub fn dismiss(&self, serial: u64) {
        PROTOCOL.notify_with(&self.seam, DISMISS, &Dismissal { serial });
    }

    pub fn arrange(&self, serial: u64, rows: Rows) {
        PROTOCOL.notify_with(&self.seam, ARRANGE, &Arrangement { serial, rows });
    }
}

#[must_use]
pub fn changed(event: &Event) -> Option<Option<Switch>> {
    PROTOCOL.decode_event(event, CHANGED)
}

pub fn states(seam: &SeamClient) -> impl Stream<Item = Option<Switch>> + Send + 'static {
    PROTOCOL.states(seam, CHANGED, STATE)
}
