use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};

use crate::host;

const UPSTREAM: &str = "origin/main";

pub(crate) fn require_published(workspace: &Path) -> Result<()> {
    let changed = host::output(git(workspace)?.args(["status", "--porcelain"]))?;
    if !changed.trim().is_empty() {
        bail!("the checkout has uncommitted changes; commit and push them before releasing");
    }
    host::run(git(workspace)?.args(["fetch", "--quiet", "origin"]))?;
    let pushed = git(workspace)?
        .args(["merge-base", "--is-ancestor", "HEAD", UPSTREAM])
        .status()?
        .success();
    if !pushed {
        bail!(
            "HEAD is not on {UPSTREAM}; push it before releasing so every build names a public revision"
        );
    }
    Ok(())
}

fn git(workspace: &Path) -> Result<Command> {
    let mut command = host::clean_command(&host::require("git")?);
    command.current_dir(workspace);
    Ok(command)
}
