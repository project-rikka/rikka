use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::gecko::tree::Tree;
use crate::host;

const SUFFIX: &str = ".patch";

pub(crate) struct Patches {
    root: PathBuf,
}

impl Patches {
    pub(crate) fn new(root: PathBuf) -> Patches {
        Patches { root }
    }

    pub(crate) fn stamp(&self, commit: &str) -> Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(commit.as_bytes());
        for file in self.listed()? {
            hasher.update(b"\0");
            hasher.update(file.as_bytes());
            hasher.update(b"\0");
            hasher.update(read_bytes(&self.patch(&file))?);
        }
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    pub(crate) fn apply(&self, tree: &Tree) -> Result<()> {
        tree.reset()?;
        for file in self.listed()? {
            tree.apply(&self.patch(&file))?;
        }
        Ok(())
    }

    pub(crate) fn export(&self, tree: &Tree, project: &str, held: &BTreeSet<String>) -> Result<()> {
        let changed: BTreeSet<String> = tree.changed(project)?.into_iter().collect();
        for file in changed.difference(held) {
            self.export_file(tree, file)?;
        }
        for file in self.listed()? {
            if !changed.contains(&file) && !held.contains(&file) {
                eprintln!("dropping the patch of {file}, which no longer differs from Firefox");
                host::remove(&self.patch(&file))?;
            }
        }
        remove_empty_directories(&self.root)
    }

    fn export_file(&self, tree: &Tree, file: &str) -> Result<()> {
        let source = tree.root().join(file);
        if source.is_file() && has_conflict_markers(&read_bytes(&source)?) {
            bail!("{} still has conflict markers", source.display());
        }
        let patch = self.patch(file);
        if patch.is_file() && tree.reproduces(&patch, file)? {
            return Ok(());
        }
        eprintln!("writing the patch of {file}");
        host::write(&patch, tree.diff(file)?)
    }

    fn listed(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();
        if self.root.is_dir() {
            collect(&self.root, &self.root, &mut files)?;
        }
        files.sort();
        Ok(files)
    }

    fn patch(&self, file: &str) -> PathBuf {
        self.root.join(format!("{file}{SUFFIX}"))
    }
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
    {
        let path = entry
            .with_context(|| format!("failed to list {}", directory.display()))?
            .path();
        if path.is_dir() {
            collect(root, &path, files)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .context("listed entries lie under their root")?
            .to_str()
            .with_context(|| format!("{} is not UTF-8", path.display()))?;
        if let Some(file) = relative.strip_suffix(SUFFIX) {
            files.push(file.to_owned());
        }
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))
}

fn has_conflict_markers(contents: &[u8]) -> bool {
    contents.split(|&byte| byte == b'\n').any(|line| {
        [b"<<<<<<<", b">>>>>>>"].iter().any(|marker| {
            line.strip_prefix(marker.as_slice())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(b" "))
        })
    })
}

fn remove_empty_directories(directory: &Path) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
    {
        let path = entry
            .with_context(|| format!("failed to list {}", directory.display()))?
            .path();
        if !path.is_dir() {
            continue;
        }
        remove_empty_directories(&path)?;
        let empty = std::fs::read_dir(&path)
            .with_context(|| format!("failed to list {}", path.display()))?
            .next()
            .is_none();
        if empty {
            host::remove(&path)?;
        }
    }
    Ok(())
}
