use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::error::{Error, Result};
use crate::options::Limit;
use crate::path::{contained, key, relative_parts, sibling};
use crate::unfollowed;

pub trait Resolver: Send + Sync {
    fn read(&self, uri: &str, limit: u64) -> Result<Arc<[u8]>>;
}

#[derive(Debug)]
pub struct DirectoryResolver {
    root: PathBuf,
}

impl DirectoryResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn locate(&self, uri: &str) -> Result<PathBuf> {
        let parts =
            relative_parts(uri).ok_or_else(|| Error::read(uri, "path escapes model directory"))?;
        let mut path = self.root.clone();
        for part in parts {
            let direct = path.join(&part);
            path = match direct.exists() {
                true => direct,
                false => match sibling(&path, &part) {
                    Some(sibling_path) => sibling_path,
                    None => return Err(Error::read(uri, "no such file")),
                },
            };
        }
        contained(&self.root, &path).ok_or_else(|| Error::read(uri, "path escapes model directory"))
    }
}

impl Resolver for DirectoryResolver {
    fn read(&self, uri: &str, limit: u64) -> Result<Arc<[u8]>> {
        let path = self.locate(uri)?;
        let unreadable = |error: std::io::Error| Error::read(uri, error.to_string());
        let file = unfollowed::open(&path).map_err(unreadable)?;
        let metadata = file.metadata().map_err(unreadable)?;
        if !metadata.is_file() {
            return Err(Error::read(uri, "not a regular file"));
        }
        Limit::ResourceBytes.check(metadata.len(), limit)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(unreadable)?;
        Limit::ResourceBytes.check(bytes.len() as u64, limit)?;
        Ok(Arc::from(bytes))
    }
}

#[derive(Debug, Default)]
pub struct MemoryResolver {
    files: FxHashMap<String, Arc<[u8]>>,
}

impl MemoryResolver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, uri: &str, bytes: impl Into<Arc<[u8]>>) {
        self.files.insert(key(uri), bytes.into());
    }

    #[must_use]
    pub fn with_file(mut self, uri: &str, bytes: impl Into<Arc<[u8]>>) -> Self {
        self.insert(uri, bytes);
        self
    }
}

impl Resolver for MemoryResolver {
    fn read(&self, uri: &str, limit: u64) -> Result<Arc<[u8]>> {
        let bytes = self
            .files
            .get(&key(uri))
            .ok_or_else(|| Error::read(uri, "no in-memory resource with that name"))?;
        Limit::ResourceBytes.check(bytes.len() as u64, limit)?;
        Ok(Arc::clone(bytes))
    }
}

#[derive(Debug, Default)]
pub struct NoResolver;

impl Resolver for NoResolver {
    fn read(&self, uri: &str, _limit: u64) -> Result<Arc<[u8]>> {
        Err(Error::read(uri, "import has no resource resolver"))
    }
}
