use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::dist::{GECKO_PIN, GeckoPin, RELEASE, Release};
use crate::host;

pub(crate) const REPOSITORY: &str = "project-rikka/packages";

const BRANCH: &str = "main";

const START: &str = "Start the distribution";

pub(crate) struct Checkout {
    root: PathBuf,
}

impl Checkout {
    pub(crate) fn fresh(root: &Path) -> Result<Checkout> {
        host::remove(root)?;
        host::create_parent(root)?;
        host::run(
            host::clean_command(&host::require("gh")?)
                .args(["repo", "clone", REPOSITORY])
                .arg(root)
                .args(["--", "--quiet"]),
        )?;
        let checkout = Checkout {
            root: root.to_owned(),
        };
        host::run(checkout.git()?.args(["checkout", "--quiet", "-B", BRANCH]))?;
        Ok(checkout)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn release(&self) -> Result<Release> {
        Release::read_or_new(&self.root.join(RELEASE))
    }

    pub(crate) fn history(&self, generations: usize) -> Result<Vec<Release>> {
        let mut releases = Vec::new();
        for back in 0..generations {
            let Some(text) = self.shown(back, RELEASE)? else {
                break;
            };
            releases.push(
                serde_json::from_str(&text)
                    .with_context(|| format!("HEAD~{back}:{RELEASE} does not parse"))?,
            );
        }
        Ok(releases)
    }

    pub(crate) fn gecko_history(&self, generations: usize) -> Result<Vec<GeckoPin>> {
        let mut pins = Vec::new();
        for back in 0..generations {
            if let Some(text) = self.shown(back, GECKO_PIN)? {
                pins.push(GeckoPin::parse(&text)?);
            }
        }
        Ok(pins)
    }

    fn shown(&self, back: usize, file: &str) -> Result<Option<String>> {
        let shown = self
            .git()?
            .args(["show", &format!("HEAD~{back}:{file}")])
            .output()
            .context("failed to run git show")?;
        if !shown.status.success() {
            return Ok(None);
        }
        String::from_utf8(shown.stdout)
            .map(Some)
            .with_context(|| format!("HEAD~{back}:{file} is not UTF-8"))
    }

    pub(crate) fn ensure_started(&self) -> Result<()> {
        let started = self
            .git()?
            .args(["rev-parse", "--verify", "--quiet", "HEAD"])
            .output()
            .context("failed to run git rev-parse")?
            .status
            .success();
        if started {
            return Ok(());
        }
        host::run(
            self.git()?
                .args(["commit", "--quiet", "--allow-empty", "-m", START]),
        )?;
        self.push()
    }

    pub(crate) fn publish(&self, message: &str) -> Result<bool> {
        host::run(self.git()?.args(["add", "--all"]))?;
        let unchanged = self
            .git()?
            .args(["diff", "--cached", "--quiet"])
            .status()
            .context("failed to run git diff")?
            .success();
        if unchanged {
            return Ok(false);
        }
        host::run(self.git()?.args(["commit", "--quiet", "-m", message]))?;
        self.push()?;
        Ok(true)
    }

    fn push(&self) -> Result<()> {
        host::run(
            self.git()?
                .args(["-c", "credential.helper=", "-c"])
                .arg("credential.helper=!gh auth git-credential")
                .args(["push", "--quiet", "origin", BRANCH]),
        )
    }

    fn git(&self) -> Result<Command> {
        let mut command = host::clean_command(&host::require("git")?);
        command.current_dir(&self.root);
        Ok(command)
    }
}
