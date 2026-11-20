use std::time::SystemTime;

use crate::config;
use crate::error::{Error, Result};
use crate::failure::Failure;
use crate::git::git;
use crate::progress::{FETCH, PULL, PUSH, Progress};
use crate::repo::Repo;
use crate::status::Status;

const DEFAULT_REMOTE: &str = "origin";

const FETCH_HEAD: &str = "FETCH_HEAD";

const MERGE_HEAD: &str = "MERGE_HEAD";

const REBASE_DIRS: [&str; 2] = ["rebase-merge", "rebase-apply"];

const REMOTES: &str = r"^(remote\..+\.url|branch\..+\.remote)$";

const PULL_STRATEGIES: &str = r"^(pull\.(rebase|ff)|branch\..+\.rebase)$";

const PULL_SETTINGS: [&str; 2] = ["pull.rebase", "pull.ff"];

enum Stop {
    Merge,
    Rebase,
}

impl Repo {
    pub fn fetch(&self, remote: &str, report: impl FnMut(Progress)) -> Result<()> {
        git(&self.root)
            .args(["fetch", "--progress", "--prune", "--"])
            .arg(remote)
            .transfer(&FETCH, report)
            .map_err(Failure::into_error)
    }

    pub fn pull(&self, status: &Status, report: impl FnMut(Progress)) -> Result<()> {
        let branch = status.branch().ok_or(Error::Headless)?;
        if status.upstream().is_none() || status.is_upstream_gone() {
            return Err(Error::Upstreamless);
        }
        let before = self.stop();
        let pulling = git(&self.root).args(["pull", "--progress"]);
        let pulling = match self.has_pull_strategy(branch) {
            true => pulling,
            false => pulling.arg("--no-rebase"),
        };
        pulling
            .transfer(&PULL, report)
            .map_err(|failure| match (before, self.stop()) {
                (None, Some(Stop::Merge)) => Error::Conflicts,
                (None, Some(Stop::Rebase)) => Error::RebaseConflicts,
                _ => failure.into_error(),
            })
    }

    pub fn push(&self, status: &Status, report: impl FnMut(Progress)) -> Result<()> {
        let branch = status.branch().ok_or(Error::Headless)?;
        let pushing = git(&self.root).args(["push", "--progress"]);
        let pushing = match status.upstream().is_some() && !status.is_upstream_gone() {
            true => pushing,
            false => {
                let remote = self.remote(Some(branch)).ok_or(Error::Remoteless)?;
                pushing
                    .args(["--set-upstream", "--"])
                    .arg(remote)
                    .arg(branch)
            }
        };
        pushing
            .transfer(&PUSH, report)
            .map_err(|failure| match failure.is_rejected() {
                true => Error::Rejected,
                false => failure.into_error(),
            })
    }

    pub(crate) fn remote(&self, branch: Option<&str>) -> Option<String> {
        let output = config::matching(&self.root, REMOTES)
            .inspect_err(|error| {
                tracing::debug!(%error, path = %self.root.display(), "failed to read remotes");
            })
            .ok()?;
        let mut remotes = Vec::new();
        let mut tracked = None;
        for (key, value) in config::entries(&output) {
            if let Some(name) = key
                .strip_prefix("remote.")
                .and_then(|key| key.strip_suffix(".url"))
            {
                remotes.push(name);
            } else if branch.is_some()
                && key
                    .strip_prefix("branch.")
                    .and_then(|key| key.strip_suffix(".remote"))
                    == branch
            {
                tracked = Some(value);
            }
        }
        tracked
            .filter(|name| remotes.contains(name))
            .or_else(|| remotes.iter().copied().find(|name| *name == DEFAULT_REMOTE))
            .or_else(|| remotes.first().copied())
            .map(str::to_owned)
    }

    pub(crate) fn fetched(&self) -> Option<SystemTime> {
        [&self.git_dir, &self.common_dir]
            .into_iter()
            .filter_map(|dir| dir.join(FETCH_HEAD).metadata().ok()?.modified().ok())
            .max()
    }

    fn has_pull_strategy(&self, branch: &str) -> bool {
        let rebase = format!("branch.{branch}.rebase");
        match config::matching(&self.root, PULL_STRATEGIES) {
            Ok(output) => config::entries(&output)
                .any(|(key, _)| PULL_SETTINGS.contains(&key) || key == rebase),
            Err(error) => {
                tracing::debug!(%error, path = %self.root.display(), "failed to read pull settings");
                true
            }
        }
    }

    fn stop(&self) -> Option<Stop> {
        if REBASE_DIRS
            .iter()
            .any(|dir| self.git_dir.join(dir).is_dir())
        {
            return Some(Stop::Rebase);
        }
        self.git_dir
            .join(MERGE_HEAD)
            .is_file()
            .then_some(Stop::Merge)
    }
}
