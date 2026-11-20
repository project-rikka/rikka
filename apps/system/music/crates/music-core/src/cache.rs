use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

use crate::error::{Error, Result};

const SPOOLS: &str = ".spool";
const HIDDEN: char = '.';

static SPOOLED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct AudioCache {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    root: PathBuf,
    limit: AtomicU64,
    tidying: AtomicBool,
    again: AtomicBool,
}

#[derive(Clone, Debug)]
pub struct ProviderCache {
    cache: AudioCache,
    folder: PathBuf,
    spools: PathBuf,
}

struct CacheEntry {
    path: PathBuf,
    size: u64,
    used: SystemTime,
}

impl AudioCache {
    #[must_use]
    pub fn open(root: impl Into<PathBuf>, limit: u64) -> AudioCache {
        let cache = AudioCache {
            inner: Arc::new(Inner {
                root: root.into(),
                limit: AtomicU64::new(limit),
                tidying: AtomicBool::new(false),
                again: AtomicBool::new(false),
            }),
        };
        sweep(&cache.inner.root.join(SPOOLS));
        cache.prune();
        cache
    }

    #[must_use]
    pub fn for_provider(&self, slug: &str) -> ProviderCache {
        let name = file_name(slug);
        ProviderCache {
            cache: self.clone(),
            folder: self.inner.root.join(&name),
            spools: self.inner.root.join(SPOOLS).join(name),
        }
    }

    #[must_use]
    pub fn limit(&self) -> u64 {
        self.inner.limit.load(Ordering::Relaxed)
    }

    pub fn set_limit(&self, bytes: u64) {
        self.inner.limit.store(bytes, Ordering::Relaxed);
        self.prune();
    }

    #[must_use]
    pub fn prune_and_measure(&self) -> u64 {
        prune(&self.inner.root, self.limit())
    }

    pub fn clear(&self) -> Result<()> {
        let entries = match fs::read_dir(&self.inner.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(disk(error)),
        };
        for entry in entries.flatten() {
            if hidden(&entry.file_name()) {
                continue;
            }
            let path = entry.path();
            let removed = match entry.file_type().map(|kind| kind.is_dir()) {
                Ok(true) => fs::remove_dir_all(&path),
                _ => fs::remove_file(&path),
            };
            removed.map_err(disk)?;
        }
        Ok(())
    }

    pub fn prune(&self) {
        if self.inner.tidying.swap(true, Ordering::AcqRel) {
            self.inner.again.store(true, Ordering::Release);
            return;
        }
        let inner = Arc::clone(&self.inner);
        let started = std::thread::Builder::new()
            .name("rikka-music-cache".into())
            .spawn(move || {
                loop {
                    inner.again.store(false, Ordering::Release);
                    prune(&inner.root, inner.limit.load(Ordering::Relaxed));
                    if inner.again.load(Ordering::Acquire) {
                        continue;
                    }
                    inner.tidying.store(false, Ordering::Release);
                    let missed = inner.again.load(Ordering::Acquire);
                    if !missed || inner.tidying.swap(true, Ordering::AcqRel) {
                        break;
                    }
                }
            });
        if let Err(error) = started {
            self.inner.tidying.store(false, Ordering::Release);
            tracing::warn!("failed to prune audio cache: {error}");
        }
    }
}

impl ProviderCache {
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    #[must_use]
    pub fn find(&self, key: &str) -> Option<PathBuf> {
        let path = self.folder.join(file_name(key));
        touch(&path).ok().map(|()| path)
    }

    pub fn insert(&self, key: &str, staged: &Path) -> Result<()> {
        if self.cache.limit() == 0 {
            return fs::remove_file(staged).map_err(disk);
        }
        fs::create_dir_all(&self.folder).map_err(disk)?;
        let path = self.folder.join(file_name(key));
        if fs::rename(staged, &path).is_err() {
            fs::copy(staged, &path).map_err(disk)?;
            fs::remove_file(staged).map_err(disk)?;
        }
        touch(&path).map_err(disk)?;
        self.cache.prune();
        Ok(())
    }

    pub fn spool(&self) -> Result<PathBuf> {
        let folder = self.spools.join(format!(
            "{}-{}",
            std::process::id(),
            SPOOLED.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&folder).map_err(disk)?;
        Ok(folder)
    }

    pub fn prune(&self) {
        self.cache.prune();
    }
}

fn file_name(key: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut name = String::with_capacity(key.len());
    for byte in key.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' => name.push(char::from(byte)),
            _ => {
                name.push('_');
                name.push(char::from(HEX[usize::from(byte >> 4)]));
                name.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    if name.starts_with(HIDDEN) {
        name.replace_range(..1, "_2e");
    }
    name
}

fn hidden(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_none_or(|name| name.starts_with(HIDDEN))
}

fn touch(path: &Path) -> std::io::Result<()> {
    File::options()
        .write(true)
        .open(path)?
        .set_modified(SystemTime::now())
}

fn entries(root: &Path) -> Vec<CacheEntry> {
    let mut cache_entries = Vec::new();
    let mut folders = vec![root.to_owned()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            if hidden(&entry.file_name()) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                folders.push(entry.path());
                continue;
            }
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let accessed = metadata.accessed().unwrap_or(modified);
            cache_entries.push(CacheEntry {
                path: entry.path(),
                size: metadata.len(),
                used: modified.max(accessed),
            });
        }
    }
    cache_entries
}

fn prune(root: &Path, limit: u64) -> u64 {
    let mut cache_entries = entries(root);
    let mut total: u64 = cache_entries.iter().map(|entry| entry.size).sum();
    cache_entries.sort_by_key(|entry| entry.used);
    for oldest in cache_entries {
        if total <= limit {
            break;
        }
        match fs::remove_file(&oldest.path) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                tracing::debug!("failed to evict {}: {error}", oldest.path.display());
            }
            _ => total = total.saturating_sub(oldest.size),
        }
    }
    total
}

fn sweep(spools: &Path) {
    let Ok(providers) = fs::read_dir(spools) else {
        return;
    };
    for provider in providers.flatten() {
        let Ok(entries) = fs::read_dir(provider.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let owner = name
                .to_str()
                .and_then(|name| name.split_once('-'))
                .map(|(pid, _)| pid);
            let is_orphaned = owner.is_none_or(|pid| !Path::new("/proc").join(pid).exists());
            if is_orphaned {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}

fn disk(error: impl std::fmt::Display) -> Error {
    Error::Disk(error.to_string())
}
