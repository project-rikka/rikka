use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use anyhow::{Context, Result};

use crate::host;

pub(crate) fn mirror(source: &Path, destination: &Path, kept: &[&str]) -> Result<()> {
    std::fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    let mut present = BTreeSet::new();
    for entry in entries(source)? {
        let name = entry.file_name().context("listed entries have names")?;
        present.insert(name.to_os_string());
        let target = destination.join(name);
        if entry.is_dir() {
            mirror(&entry, &target, &[])?;
        } else {
            copy_changed(&entry, &target)?;
        }
    }
    let kept: BTreeSet<OsString> = kept.iter().map(OsString::from).collect();
    for entry in entries(destination)? {
        let name = entry.file_name().context("listed entries have names")?;
        if !present.contains(name) && !kept.contains(name) {
            host::remove(&entry)?;
        }
    }
    Ok(())
}

fn copy_changed(source: &Path, target: &Path) -> Result<()> {
    if same_contents(source, target)? {
        return Ok(());
    }
    host::copy(source, target)
}

fn entries(directory: &Path) -> Result<Vec<std::path::PathBuf>> {
    std::fs::read_dir(directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .with_context(|| format!("failed to list {}", directory.display()))
        })
        .collect()
}

fn same_contents(source: &Path, target: &Path) -> Result<bool> {
    if !target.is_file() {
        return Ok(false);
    }
    let read = |path: &Path| {
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))
    };
    Ok(read(source)? == read(target)?)
}
