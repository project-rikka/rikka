use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use crate::bazel::{self, Profile};
use crate::host;

const OUTPUT_ROOT: &str = "bazel-out";

const OUTPUT_DEPTH: usize = 3;

const GENERATED: &str = "generated";

pub(crate) fn refresh(workspace: &Path, target: &str) -> Result<()> {
    let built = bazel::build(workspace, Profile::Dev, [target])?;
    for output in built.files(target)? {
        let destination = workspace.join(committed(output)?);
        host::copy(output, &destination)?;
        host::set_mode(&destination, 0o644)?;
        println!("wrote {}", destination.display());
    }
    Ok(())
}

fn committed(output: &Path) -> Result<PathBuf> {
    let parts: Vec<&str> = output
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let package = parts
        .iter()
        .position(|part| *part == OUTPUT_ROOT)
        .map(|position| position + OUTPUT_DEPTH)
        .filter(|package| *package < parts.len())
        .with_context(|| format!("{} is not a Bazel output", output.display()))?;
    let generated = parts[package..]
        .iter()
        .position(|part| *part == GENERATED)
        .map(|position| package + position)
        .with_context(|| format!("{} is not under a {GENERATED} folder", output.display()))?;
    Ok(parts[package..generated]
        .iter()
        .chain(&parts[generated + 1..])
        .collect())
}
