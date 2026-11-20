use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use shell_kit::StateFile;

const FILE: &str = "launcher.json";

const WEIGHT: f32 = 2.0;

const CAPACITY: usize = 512;

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Scale {
    now: u64,
    best: f32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct Entry {
    count: u32,
    last: u64,
}

impl Entry {
    fn frecency(&self, now: u64) -> f32 {
        let age = now.saturating_sub(self.last);
        let decay = if age < HOUR {
            4.0
        } else if age < DAY {
            2.0
        } else if age < WEEK {
            0.5
        } else {
            0.25
        };
        self.count as f32 * decay
    }
}

#[derive(Debug, Default, serde::Deserialize)]
struct Document {
    entries: HashMap<String, Entry>,
}

#[derive(serde::Serialize)]
struct Written<'a> {
    entries: &'a HashMap<String, Entry>,
}

#[derive(Debug, Default)]
pub(crate) struct Usage {
    entries: HashMap<String, Entry>,
    file: Option<StateFile>,
}

impl Usage {
    #[must_use]
    pub(crate) fn read() -> Usage {
        let Some(file) = StateFile::state(FILE) else {
            return Usage::default();
        };
        Usage {
            entries: file.load_json::<Document>().entries,
            file: Some(file),
        }
    }

    pub(crate) fn scale(&self) -> Scale {
        let now = now();
        Scale {
            now,
            best: self
                .entries
                .values()
                .map(|entry| entry.frecency(now))
                .fold(0.0_f32, f32::max),
        }
    }

    pub(crate) fn lift(&self, key: &str, scale: Scale) -> f32 {
        if scale.best <= 0.0 {
            return 0.0;
        }
        let Some(entry) = self.entries.get(key) else {
            return 0.0;
        };
        WEIGHT * (entry.frecency(scale.now) / scale.best)
    }

    pub(super) fn last_used(&self, key: &str) -> Option<u64> {
        self.entries.get(key).map(|entry| entry.last)
    }

    pub(super) fn best(&self, limit: usize) -> Vec<&str> {
        let now = now();
        let mut ranked: Vec<(&str, f32)> = self
            .entries
            .iter()
            .map(|(key, entry)| (key.as_str(), entry.frecency(now)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        ranked.truncate(limit);
        ranked.into_iter().map(|(key, _)| key).collect()
    }

    pub(crate) fn record(&mut self, key: &str) {
        let now = now();
        let entry = self.entries.entry(key.to_owned()).or_insert(Entry {
            count: 0,
            last: now,
        });
        entry.count = entry.count.saturating_add(1);
        entry.last = now;
        if self.entries.len() > CAPACITY {
            self.forget(now);
        }
        self.write()
    }

    fn forget(&mut self, now: u64) {
        let worst = self
            .entries
            .iter()
            .min_by(|a, b| a.1.frecency(now).total_cmp(&b.1.frecency(now)))
            .map(|(key, _)| key.clone());
        if let Some(worst) = worst {
            self.entries.remove(&worst);
        }
    }

    fn write(&self) {
        let Some(file) = &self.file else {
            return;
        };
        let document = Written {
            entries: &self.entries,
        };
        match serde_json::to_string(&document) {
            Ok(document) => file.save(document),
            Err(error) => tracing::warn!("failed to encode launcher history: {error}"),
        }
    }
}

impl Drop for Usage {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            file.flush();
        }
    }
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}
