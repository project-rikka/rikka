use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use rikka_backup::{BRANCH, Configured, checked_url, clone_arguments};
use rikka_fs::Replace;

use crate::error::{Error, Result};

const REMOTE: &str = "origin";
const TRACKED: &str = "refs/remotes/origin/main";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Published {
    Pushed,
    Unchanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    folder: PathBuf,
}

impl Repository {
    #[must_use]
    pub fn new(folder: impl Into<PathBuf>) -> Repository {
        Repository {
            folder: folder.into(),
        }
    }

    pub fn publish(
        &self,
        url: &str,
        modules: &[Configured],
        message: &str,
        identity: &Identity,
    ) -> Result<Published> {
        let url = checked_url(url)?;
        std::fs::create_dir_all(&self.folder).map_err(Error::io(&self.folder))?;
        if !self.folder.join(".git").exists() {
            self.git(["init", "--quiet", "--initial-branch", BRANCH])?;
        }
        match self.git(["remote", "get-url", REMOTE]) {
            Ok(_) => self.git(["remote", "set-url", REMOTE, url])?,
            Err(_) => self.git(["remote", "add", REMOTE, url])?,
        };
        self.git(["fetch", "--quiet", REMOTE])?;
        if self
            .git(["rev-parse", "--verify", "--quiet", TRACKED])
            .is_ok()
        {
            self.git(["reset", "--quiet", "--mixed", TRACKED])?;
        }
        for configured in modules {
            let path = self.folder.join(configured.module.file_name());
            Replace::new(&path)
                .write(configured.text.as_bytes())
                .map_err(Error::io(&path))?;
        }
        self.git(["add", "--all"])?;
        let staged = self.run(["diff", "--cached", "--quiet"])?;
        let born = self
            .git(["rev-parse", "--verify", "--quiet", "HEAD"])
            .is_ok();
        if staged.status.success() && born {
            return Ok(Published::Unchanged);
        }
        let mut commit = self.command(["commit", "--quiet", "--message", message]);
        if self.git(["config", "user.email"]).is_err() {
            commit
                .env("GIT_AUTHOR_NAME", &identity.name)
                .env("GIT_AUTHOR_EMAIL", &identity.email)
                .env("GIT_COMMITTER_NAME", &identity.name)
                .env("GIT_COMMITTER_EMAIL", &identity.email);
        }
        checked("commit", commit.output())?;
        let destination = format!("HEAD:refs/heads/{BRANCH}");
        self.git(["push", "--quiet", REMOTE, destination.as_str()])?;
        Ok(Published::Pushed)
    }

    pub fn fetch(url: &str, into: &Path) -> Result<Vec<Configured>> {
        let target = into
            .to_str()
            .ok_or_else(|| Error::Unnamed(into.to_owned()))?;
        let mut clone = git_command();
        clone.args(clone_arguments(url, target)?);
        checked("clone", clone.output())?;
        Ok(Configured::read_folder(into)?)
    }

    fn git<'a>(&self, args: impl IntoIterator<Item = &'a str>) -> Result<String> {
        let args: Vec<&str> = args.into_iter().collect();
        let name = args.first().copied().unwrap_or_default().to_owned();
        let output = checked(&name, self.command(args).output())?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn run<'a>(&self, args: impl IntoIterator<Item = &'a str>) -> Result<Output> {
        let args: Vec<&str> = args.into_iter().collect();
        let name = args.first().copied().unwrap_or_default().to_owned();
        self.command(args).output().map_err(|error| Error::Git {
            command: name,
            reason: error.to_string(),
        })
    }

    fn command<I: AsRef<OsStr>>(&self, args: impl IntoIterator<Item = I>) -> Command {
        let mut command = git_command();
        command.arg("-C").arg(&self.folder).args(args);
        command
    }
}

fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C");
    command
}

fn checked(command: &str, output: std::io::Result<Output>) -> Result<Output> {
    let output = output.map_err(|error| Error::Git {
        command: command.to_owned(),
        reason: error.to_string(),
    })?;
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(Error::Git {
        command: command.to_owned(),
        reason: match stderr.is_empty() {
            true => output.status.to_string(),
            false => stderr,
        },
    })
}
