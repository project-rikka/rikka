use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) type Level = Arc<AtomicU32>;

#[derive(Debug, Clone, Default)]
pub struct Peaks {
    levels: Arc<Mutex<HashMap<u32, Level>>>,
}

impl Peaks {
    pub fn level(&self, node: u32) -> f32 {
        self.levels()
            .get(&node)
            .map_or(0.0, |level| f32::from_bits(level.load(Ordering::Relaxed)))
    }

    pub(crate) fn watch(&self, node: u32) -> Level {
        let level = Level::default();
        self.levels().insert(node, Arc::clone(&level));
        level
    }

    pub(crate) fn only(&self, nodes: &[u32]) {
        self.levels().retain(|node, _| nodes.contains(node));
    }

    fn levels(&self) -> rikka_sync::MutexGuard<'_, HashMap<u32, Level>> {
        self.levels.lock()
    }
}
