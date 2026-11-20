use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::host;
use crate::release::checkout::REPOSITORY;
use crate::release::github::gh;

const TAG: &str = "binaries";

pub(crate) const DOWNLOAD: &str =
    "https://github.com/project-rikka/packages/releases/download/binaries";

pub(crate) fn ensure() -> Result<()> {
    let exists = gh()?
        .args(["release", "view", TAG, "--repo", REPOSITORY])
        .output()
        .context("failed to run gh release view")?
        .status
        .success();
    if exists {
        return Ok(());
    }
    host::run(gh()?.args([
        "release",
        "create",
        TAG,
        "--repo",
        REPOSITORY,
        "--title",
        "Binaries",
        "--notes",
        "The archives release.json names.",
    ]))
}

pub(crate) fn assets() -> Result<BTreeSet<String>> {
    let listed = host::output(gh()?.args([
        "release",
        "view",
        TAG,
        "--repo",
        REPOSITORY,
        "--json",
        "assets",
        "--jq",
        ".assets[].name",
    ]))?;
    Ok(listed
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

pub(crate) fn upload(files: &[PathBuf]) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    host::run(
        gh()?
            .args(["release", "upload", TAG, "--repo", REPOSITORY, "--clobber"])
            .args(files),
    )
}

pub(crate) fn delete(name: &str) -> Result<()> {
    host::run(gh()?.args([
        "release",
        "delete-asset",
        TAG,
        name,
        "--repo",
        REPOSITORY,
        "--yes",
    ]))
}
