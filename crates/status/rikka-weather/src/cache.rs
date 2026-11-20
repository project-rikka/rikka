use std::time::{Duration, SystemTime};

use rikka_net::DiskCache;

const FOLDER: &str = "weather";

const KEPT_FOR: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CacheEntry {
    pub(crate) fetched: u64,
    pub(crate) body: String,
}

impl CacheEntry {
    pub(crate) fn new(fetched: SystemTime, body: String) -> CacheEntry {
        CacheEntry {
            fetched: fetched
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
            body,
        }
    }

    pub(crate) fn fetched(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.fetched)
    }

    pub(crate) fn younger_than(&self, age: Duration, now: SystemTime) -> bool {
        now.duration_since(self.fetched())
            .is_ok_and(|elapsed| elapsed < age)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Cache {
    disk: Option<DiskCache>,
}

impl Cache {
    pub(crate) fn open() -> Cache {
        let disk = match rikka_dirs::Paths::resolve() {
            Ok(paths) => DiskCache::open(paths.cache_dir().join(FOLDER), KEPT_FOR)
                .inspect_err(|error| tracing::debug!(%error, "weather cache unavailable"))
                .ok(),
            Err(error) => {
                tracing::debug!(%error, "weather cache unavailable");
                None
            }
        };
        Cache { disk }
    }

    pub(crate) fn read(&self, key: &str) -> Option<CacheEntry> {
        let bytes = self.disk.as_ref()?.read(key)?;
        serde_json::from_slice(&bytes).ok()
    }

    pub(crate) fn write(&self, key: &str, entry: &CacheEntry) {
        let Some(disk) = &self.disk else {
            return;
        };
        let encoded = match serde_json::to_vec(entry) {
            Ok(encoded) => encoded,
            Err(error) => return tracing::debug!(%error, key, "failed to encode weather"),
        };
        if let Err(error) = disk.write(key, &encoded) {
            tracing::debug!(%error, key, "failed to cache weather");
        }
    }
}

pub(crate) fn key(kind: &str, latitude: f64, longitude: f64) -> String {
    format!("{kind}-{latitude:.2}_{longitude:.2}")
}
