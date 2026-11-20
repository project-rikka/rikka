use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Arrivals {
    seen: BTreeMap<String, Option<u64>>,
}

impl Arrivals {
    pub fn observe<'a>(&mut self, present: impl IntoIterator<Item = &'a str>, now: u64) -> bool {
        let present: HashSet<&str> = present.into_iter().collect();
        if present.is_empty() {
            return false;
        }
        let stamp = (!self.seen.is_empty()).then_some(now);
        let known = self.seen.len();
        self.seen.retain(|id, _| present.contains(id.as_str()));
        let mut changed = self.seen.len() != known;
        for id in present {
            if !self.seen.contains_key(id) {
                self.seen.insert(id.to_owned(), stamp);
                changed = true;
            }
        }
        changed
    }

    #[must_use]
    pub fn arrived(&self, id: &str) -> Option<u64> {
        self.seen.get(id).copied().flatten()
    }
}
