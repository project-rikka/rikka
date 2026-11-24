use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::dist::{self, Fragment, Placement, Release};
use crate::host;

const MODULE_CHECK: &str = "checks.x86_64-linux.module.drvPath";

pub(crate) fn verify(
    workspace: &Path,
    fragments: &[Fragment],
    archives: &Path,
    dir: &Path,
) -> Result<()> {
    host::reset_dir(dir)?;
    let mut release = Release::empty();
    dist::merge(
        &mut release,
        fragments,
        Placement::Copied { from: archives },
        dir,
    )?;
    dist::write_flake(workspace, &release, dir)?;

    let flake = format!("path:{}", dir.display());
    for fragment in fragments {
        println!("verifying {}", fragment.id);
        host::run(
            nix()?
                .args(["build", "--no-link"])
                .arg(format!("{flake}#{}", fragment.id)),
        )?;
    }
    println!("evaluating a system with every component");
    host::run(
        nix()?
            .args(["eval", "--raw"])
            .arg(format!("{flake}#{MODULE_CHECK}")),
    )?;
    println!();
    Ok(())
}

fn nix() -> Result<Command> {
    let mut command = host::clean_command(&host::require("nix")?);
    command.args(["--extra-experimental-features", "nix-command flakes"]);
    Ok(command)
}
