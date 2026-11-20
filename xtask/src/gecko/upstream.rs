use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::host;

const PIN: &str = "upstream.toml";

const TAG_PREFIX: &str = "FIREFOX_";

const TAG_SUFFIX: &str = "_RELEASE";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Upstream {
    pub(crate) repository: String,
    pub(crate) version: String,
    pub(crate) commit: String,
}

impl Upstream {
    pub(crate) fn read(engine: &Path) -> Result<Upstream> {
        let path = engine.join(PIN);
        toml::from_str(&host::read(&path)?)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    pub(crate) fn write(&self, engine: &Path) -> Result<()> {
        let path = engine.join(PIN);
        let text = toml::to_string(self)
            .with_context(|| format!("failed to encode {}", path.display()))?;
        host::write(&path, text)
    }

    pub(crate) fn tag(&self) -> String {
        tag_of(&self.version)
    }

    pub(crate) fn latest(&self) -> Result<Option<Upstream>> {
        let listed = host::output(git()?.args(["ls-remote", "--tags", "--refs"]).args([
            self.repository.as_str(),
            &format!("{TAG_PREFIX}*{TAG_SUFFIX}"),
        ]))?;
        let newest = listed
            .lines()
            .filter_map(|line| line.split_once("\trefs/tags/").map(|(_, tag)| tag))
            .filter_map(release_version)
            .max_by(|left, right| left.numbers.cmp(&right.numbers))
            .with_context(|| format!("found no Firefox release tag in {}", self.repository))?;
        let current = release_version(&self.tag())
            .with_context(|| format!("{} is not a Firefox release version", self.version))?;
        if newest.numbers <= current.numbers {
            return Ok(None);
        }
        let commit = self.peeled_commit(&tag_of(&newest.text))?;
        Ok(Some(Upstream {
            repository: self.repository.clone(),
            version: newest.text,
            commit,
        }))
    }

    fn peeled_commit(&self, tag: &str) -> Result<String> {
        let reference = format!("refs/tags/{tag}");
        let peeled = format!("{reference}^{{}}");
        let listed = host::output(
            git()?
                .args(["ls-remote", self.repository.as_str()])
                .args([reference.as_str(), peeled.as_str()]),
        )?;
        let mut commit = None;
        for (object, name) in listed.lines().filter_map(|line| line.split_once('\t')) {
            if name == peeled || commit.is_none() {
                commit = Some(object.to_owned());
            }
        }
        match commit {
            Some(commit) => Ok(commit),
            None => bail!("{} has no tag {tag}", self.repository),
        }
    }
}

struct ReleaseVersion {
    text: String,
    numbers: Vec<u32>,
}

fn release_version(tag: &str) -> Option<ReleaseVersion> {
    let digits = tag.strip_prefix(TAG_PREFIX)?.strip_suffix(TAG_SUFFIX)?;
    let numbers = digits
        .split('_')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u32>>>()?;
    if numbers.len() < 2 {
        return None;
    }
    Some(ReleaseVersion {
        text: digits.replace('_', "."),
        numbers,
    })
}

fn tag_of(version: &str) -> String {
    format!("{TAG_PREFIX}{}{TAG_SUFFIX}", version.replace('.', "_"))
}

fn git() -> Result<Command> {
    Ok(host::clean_command(&host::require("git")?))
}
