use std::path::{Path, PathBuf};

use rikka_dirs::Paths;
use rikka_types::Color;

use crate::stamp::Stamp;

const FILE: &str = "material-seeds.toml";

const MAX_ENTRIES: usize = 64;

#[derive(Debug)]
pub(crate) struct SeedCache {
    file: Option<PathBuf>,
    wallpapers: Vec<Entry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Entry {
    path: PathBuf,
    #[serde(flatten)]
    stamp: Stamp,
    source: String,
}

#[derive(Debug, Default, serde::Deserialize)]
struct CacheFile {
    #[serde(default)]
    wallpaper: Vec<Entry>,
}

#[derive(Debug, serde::Serialize)]
struct CacheFileRef<'a> {
    wallpaper: &'a [Entry],
}

impl SeedCache {
    pub(crate) fn load() -> SeedCache {
        match file() {
            Some(path) => SeedCache::open(path),
            None => SeedCache {
                file: None,
                wallpapers: Vec::new(),
            },
        }
    }

    pub(crate) fn open(file: impl Into<PathBuf>) -> SeedCache {
        let file = file.into();
        let wallpapers = match std::fs::read_to_string(&file) {
            Err(_) => Vec::new(),
            Ok(text) => toml::from_str::<CacheFile>(&text)
                .map(|read| read.wallpaper)
                .unwrap_or_else(|error| {
                    tracing::debug!(path = %file.display(), %error, "failed to parse seed cache");
                    Vec::new()
                }),
        };
        SeedCache {
            file: Some(file),
            wallpapers,
        }
    }

    pub(crate) fn get(&self, path: &Path, stamp: Stamp) -> Option<Color> {
        let entry = self
            .wallpapers
            .iter()
            .find(|entry| entry.path == path && entry.stamp == stamp)?;
        Color::parse_hex(&entry.source)
    }

    pub(crate) fn insert(&mut self, path: &Path, stamp: Stamp, source: Color) {
        self.wallpapers.retain(|entry| entry.path != path);
        self.wallpapers.push(Entry {
            path: path.to_path_buf(),
            stamp,
            source: source.to_hex(),
        });
        let excess = self.wallpapers.len().saturating_sub(MAX_ENTRIES);
        self.wallpapers.drain(..excess);
        self.write();
    }

    fn write(&self) {
        let Some(path) = &self.file else {
            return;
        };
        let result = toml::to_string_pretty(&CacheFileRef {
            wallpaper: &self.wallpapers,
        })
        .map_err(|error| error.to_string())
        .and_then(|text| std::fs::write(path, text).map_err(|error| error.to_string()));
        if let Err(error) = result {
            tracing::debug!(path = %path.display(), %error, "failed to write seed cache");
        }
    }
}

fn file() -> Option<PathBuf> {
    let paths = Paths::resolve()
        .inspect_err(|error| tracing::debug!(%error, "failed to resolve cache directory"))
        .ok()?;
    rikka_dirs::ensure(paths.cache_dir()).ok()?;
    Some(paths.cache_dir().join(FILE))
}
