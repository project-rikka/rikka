use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::host;
use crate::release::github::gh;

const HOST: &str = "https://github.com";

const LISTED_RELEASES: &str = "1000";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Tagged {
    repository: String,
    tag: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Absent,
    Draft,
    Published,
}

#[derive(Deserialize)]
struct Listed {
    #[serde(rename = "tagName")]
    tag: String,
    #[serde(rename = "isDraft")]
    draft: bool,
}

impl Tagged {
    pub(crate) fn new(repository: &str, tag: impl Into<String>) -> Tagged {
        Tagged {
            repository: repository.to_owned(),
            tag: tag.into(),
        }
    }

    pub(crate) fn of_download(url: &str) -> Option<Tagged> {
        let rest = url.strip_prefix(HOST)?.strip_prefix('/')?;
        let mut parts = rest.split('/');
        let owner = parts.next()?;
        let name = parts.next()?;
        if parts.next()? != "releases" || parts.next()? != "download" {
            return None;
        }
        let tag = parts.next()?;
        Some(Tagged::new(&format!("{owner}/{name}"), tag))
    }

    pub(crate) fn list_published(repository: &str) -> Result<Vec<Tagged>> {
        let listed = host::output(gh()?.args([
            "release",
            "list",
            "--repo",
            repository,
            "--limit",
            LISTED_RELEASES,
            "--json",
            "tagName,isDraft",
        ]))?;
        let releases: Vec<Listed> =
            serde_json::from_str(&listed).context("gh release list printed unexpected JSON")?;
        Ok(releases
            .into_iter()
            .filter(|release| !release.draft)
            .map(|release| Tagged::new(repository, release.tag))
            .collect())
    }

    pub(crate) fn download_prefix(&self) -> String {
        format!("{HOST}/{}/releases/download/{}", self.repository, self.tag)
    }

    pub(crate) fn state(&self) -> Result<State> {
        let viewed = gh()?
            .args(["release", "view", &self.tag, "--repo", &self.repository])
            .args(["--json", "isDraft", "--jq", ".isDraft"])
            .output()
            .context("failed to run gh release view")?;
        if !viewed.status.success() {
            return Ok(State::Absent);
        }
        match String::from_utf8_lossy(&viewed.stdout).trim() {
            "true" => Ok(State::Draft),
            "false" => Ok(State::Published),
            printed => bail!("gh release view printed {printed:?} for {self}"),
        }
    }

    pub(crate) fn create_draft(&self, target: &str, title: &str) -> Result<()> {
        host::run(
            gh()?
                .args(["release", "create", &self.tag, "--repo", &self.repository])
                .args(["--target", target, "--title", title])
                .args(["--generate-notes", "--draft"]),
        )
    }

    pub(crate) fn publish_latest(&self) -> Result<()> {
        host::run(gh()?.args([
            "release",
            "edit",
            &self.tag,
            "--repo",
            &self.repository,
            "--draft=false",
            "--latest",
        ]))
    }

    pub(crate) fn upload(&self, files: &[PathBuf]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }
        host::run(
            gh()?
                .args(["release", "upload", &self.tag, "--repo", &self.repository])
                .args(files),
        )
    }

    pub(crate) fn delete(&self) -> Result<()> {
        host::run(gh()?.args([
            "release",
            "delete",
            &self.tag,
            "--repo",
            &self.repository,
            "--cleanup-tag",
            "--yes",
        ]))
    }

    pub(crate) fn discard_draft(&self) -> Result<()> {
        host::run(gh()?.args([
            "release",
            "delete",
            &self.tag,
            "--repo",
            &self.repository,
            "--yes",
        ]))
    }
}

impl std::fmt::Display for Tagged {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}@{}", self.repository, self.tag)
    }
}
