use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::gecko::upstream::Upstream;
use crate::host;

const SCRATCH_INDEX: &str = ".git/rikka-export-index";

pub(crate) struct Tree {
    root: PathBuf,
}

impl Tree {
    pub(crate) fn new(root: PathBuf) -> Tree {
        Tree { root }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn check_out(&self, upstream: &Upstream) -> Result<bool> {
        let head = self.head()?;
        if head.as_deref() == Some(upstream.commit.as_str()) {
            return Ok(false);
        }
        if head.is_none() {
            host::reset_dir(&self.root)?;
            host::run(self.git()?.args(["init", "--quiet"]))?;
            host::run(
                self.git()?
                    .args(["remote", "add", "origin"])
                    .arg(&upstream.repository),
            )?;
        }
        self.move_to(&upstream.commit)?;
        Ok(true)
    }

    pub(crate) fn move_to(&self, commit: &str) -> Result<()> {
        eprintln!("fetching Firefox {commit}");
        host::run(
            self.git()?
                .args(["fetch", "--quiet", "--depth=1", "--no-tags", "origin"])
                .arg(commit),
        )?;
        host::run(
            self.git()?
                .args(["checkout", "--quiet", "--force", "--detach", "FETCH_HEAD"]),
        )
    }

    pub(crate) fn reset(&self) -> Result<()> {
        host::run(self.git()?.args(["reset", "--quiet", "--hard"]))?;
        host::run(self.git()?.args(["clean", "--quiet", "-fdx"]))
    }

    pub(crate) fn apply(&self, patch: &Path) -> Result<()> {
        host::run(
            self.git()?
                .args(["apply", "--whitespace=nowarn"])
                .arg(patch),
        )
    }

    pub(crate) fn commit_all(&self, message: &str) -> Result<String> {
        host::run(self.git()?.args(["add", "--all"]))?;
        host::run(
            self.git()?
                .args(["-c", "user.name=rikka", "-c", "user.email=rikka@localhost"])
                .args([
                    "commit",
                    "--quiet",
                    "--allow-empty",
                    "--no-verify",
                    "-m",
                    message,
                ]),
        )?;
        self.head()?.context("a commit was just made")
    }

    pub(crate) fn carry(&self, commit: &str) -> Result<Vec<String>> {
        let carried = self
            .git()?
            .args(["cherry-pick", "--no-commit", commit])
            .stdout(Stdio::from(std::io::stderr()))
            .status()
            .context("failed to run git cherry-pick")?
            .success();
        let conflicted = match carried {
            true => Vec::new(),
            false => {
                let conflicted = paths(&host::output(self.git()?.args([
                    "diff",
                    "-z",
                    "--name-only",
                    "--diff-filter=U",
                ]))?);
                if conflicted.is_empty() {
                    bail!("failed to carry {commit} onto {}", self.root.display());
                }
                conflicted
            }
        };
        host::run(self.git()?.args(["reset", "--quiet"]))?;
        Ok(conflicted)
    }

    pub(crate) fn changed(&self, excluded: &str) -> Result<Vec<String>> {
        let exclusion = format!(":(exclude){excluded}");
        let created = paths(&host::output(self.git()?.args([
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--",
            ".",
            exclusion.as_str(),
        ]))?);
        if !created.is_empty() {
            host::run(
                self.git()?
                    .args(["add", "--intent-to-add", "--"])
                    .args(&created),
            )?;
        }
        Ok(paths(&host::output(self.git()?.args([
            "diff",
            "HEAD",
            "-z",
            "--name-only",
            "--",
            ".",
            exclusion.as_str(),
        ]))?))
    }

    pub(crate) fn diff(&self, file: &str) -> Result<Vec<u8>> {
        let output = self
            .git()?
            .args(["diff", "HEAD", "--", file])
            .output()
            .context("failed to run git diff")?;
        if !output.status.success() {
            bail!(
                "git diff of {file} failed with {}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output.stdout)
    }

    pub(crate) fn reproduces(&self, patch: &Path, file: &str) -> Result<bool> {
        if !self.root.join(file).is_file() {
            return Ok(false);
        }
        let index = self.root.join(SCRATCH_INDEX);
        let expected = self.patched_blob(&index, patch, file);
        host::remove(&index)?;
        let Some(expected) = expected? else {
            return Ok(false);
        };
        let actual = host::output(self.git()?.args(["hash-object", "--", file]))?;
        Ok(expected == actual.trim())
    }

    fn patched_blob(&self, index: &Path, patch: &Path, file: &str) -> Result<Option<String>> {
        host::run(
            self.git()?
                .env("GIT_INDEX_FILE", index)
                .args(["read-tree", "HEAD"]),
        )?;
        let applied = self
            .git()?
            .env("GIT_INDEX_FILE", index)
            .args(["apply", "--cached"])
            .arg(patch)
            .output()
            .context("failed to run git apply")?
            .status
            .success();
        if !applied {
            return Ok(None);
        }
        let blob = host::output(
            self.git()?
                .env("GIT_INDEX_FILE", index)
                .args(["rev-parse", &format!(":{file}")]),
        )?;
        Ok(Some(blob.trim().to_owned()))
    }

    fn head(&self) -> Result<Option<String>> {
        if !self.root.join(".git").is_dir() {
            return Ok(None);
        }
        let output = self
            .git()?
            .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
            .output()
            .context("failed to run git rev-parse")?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    }

    fn git(&self) -> Result<Command> {
        let mut command = host::clean_command(&host::require("git")?);
        command.current_dir(&self.root);
        Ok(command)
    }
}

fn paths(listed: &str) -> Vec<String> {
    listed
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
}
