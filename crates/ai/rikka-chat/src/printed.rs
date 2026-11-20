use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rikka_ui::TextDocument;

const BUDGET: usize = 4 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct PrintedDocuments {
    cache: RefCell<PrintedCache>,
}

#[derive(Debug, Default)]
struct PrintedCache {
    models: HashMap<String, CachedDocument>,
    clock: u64,
}

#[derive(Debug)]
struct CachedDocument {
    document: TextDocument,
    stamp: u64,
    bytes: usize,
    used: u64,
}

impl PrintedDocuments {
    pub(crate) fn document(&self, call: &str, text: &str) -> TextDocument {
        let stamp = stamped(text);
        let mut cache = self.cache.borrow_mut();
        cache.clock += 1;
        let used = cache.clock;
        let document = match cache.models.get_mut(call) {
            Some(entry) => {
                entry.used = used;
                if entry.stamp != stamp {
                    entry.document.reset(text);
                    entry.stamp = stamp;
                    entry.bytes = text.len();
                }
                entry.document.clone()
            }
            None => {
                let document = TextDocument::new(text);
                cache.models.insert(
                    call.to_owned(),
                    CachedDocument {
                        document: document.clone(),
                        stamp,
                        bytes: text.len(),
                        used,
                    },
                );
                document
            }
        };
        cache.make_room();
        document
    }

    pub(crate) fn forget(&self) {
        *self.cache.borrow_mut() = PrintedCache::default();
    }
}

impl PrintedCache {
    fn make_room(&mut self) {
        let mut total: usize = self.models.values().map(|total| total.bytes).sum();
        if total <= BUDGET {
            return;
        }
        let mut order: Vec<(u64, String)> = self
            .models
            .iter()
            .map(|(call, entry)| (entry.used, call.clone()))
            .collect();
        order.sort_unstable_by_key(|(used, _)| *used);
        for (_, call) in order {
            if total <= BUDGET - BUDGET / 4 {
                return;
            }
            if let Some(gone) = self.models.remove(&call) {
                total -= gone.bytes;
            }
        }
    }
}

fn stamped(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
