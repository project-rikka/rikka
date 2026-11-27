use std::sync::{Arc, Weak};

use rikka_picture::Feed;

use crate::error::BoothError;

#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[must_use = "dropping a ticket cancels its order"]
pub struct Ticket {
    _alive: Arc<()>,
}

#[derive(Debug)]
#[must_use = "dropping a preview stops it"]
pub struct Preview {
    _alive: Arc<()>,
}

pub(crate) type Done = Box<dyn FnOnce(Result<Picture, BoothError>) + Send>;

pub(crate) enum Delivery {
    Still { alive: Weak<()>, done: Done },
    Live { alive: Weak<()>, feed: Feed },
}

impl Delivery {
    pub(crate) fn still(done: Done) -> (Self, Ticket) {
        let alive = Arc::new(());
        let delivery = Self::Still {
            alive: Arc::downgrade(&alive),
            done,
        };
        (delivery, Ticket { _alive: alive })
    }

    pub(crate) fn live(feed: Feed) -> (Self, Preview) {
        let alive = Arc::new(());
        let delivery = Self::Live {
            alive: Arc::downgrade(&alive),
            feed,
        };
        (delivery, Preview { _alive: alive })
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        match self {
            Self::Still { alive, .. } | Self::Live { alive, .. } => alive.strong_count() == 0,
        }
    }

    pub(crate) fn fail(self, error: BoothError) {
        match self {
            Self::Still { done, .. } => done(Err(error)),
            Self::Live { .. } => tracing::warn!("booth preview failed: {error}"),
        }
    }
}

impl std::fmt::Debug for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Still { .. } => f.write_str("Still"),
            Self::Live { .. } => f.write_str("Live"),
        }
    }
}
