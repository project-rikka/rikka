use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use crate::blend::BlendId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Placed {
    pub(crate) path: PathBuf,
    pub(crate) digest: u64,
}

impl Placed {
    pub(crate) fn new(path: PathBuf, bytes: &[u8]) -> Placed {
        let mut hasher = DefaultHasher::new();
        hasher.write(bytes);
        Placed {
            path,
            digest: hasher.finish(),
        }
    }
}

pub(crate) type Placements = HashMap<BlendId, Placed>;

#[derive(Clone, Debug, Default)]
pub(crate) struct Ledger(Arc<Mutex<Placements>>);

impl Ledger {
    pub(crate) fn new(placements: Placements) -> Ledger {
        Ledger(Arc::new(Mutex::new(placements)))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Placements> {
        self.0.lock()
    }
}
