use std::path::Path;

use anyhow::{Context, Result};
use rikka_web_crx::{Identity, Package, Version};

use crate::bazel::BuildOutput;
use crate::dist::release::Extension;
use crate::host;
use crate::shipped::Contents;

const CRX_SUFFIX: &str = ".crx";

pub(crate) fn stamp(workspace: &Path, version: &str) -> Result<Version> {
    let mut command = host::clean_command(&host::require("git")?);
    command
        .current_dir(workspace)
        .args(["rev-list", "--count", "HEAD"]);
    let commits: u16 = host::output(&mut command)?
        .trim()
        .parse()
        .context("the commit count does not fit a Chrome extension version part")?;
    Version::parse(&format!("{version}.{commits}")).with_context(|| {
        format!("version `{version}` is no Chrome extension version (up to three numbers)")
    })
}

pub(crate) fn sign(
    parts: &[Contents],
    built: &BuildOutput,
    stamp: Version,
    root: &Path,
) -> Result<Vec<Extension>> {
    let identity = Identity::rikka();
    let id = identity.extension_id();
    let mut signed = Vec::new();
    for extension in parts.iter().flat_map(|part| part.extensions) {
        let zip = built.only(extension.target)?;
        let bytes =
            std::fs::read(zip).with_context(|| format!("failed to read {}", zip.display()))?;
        let crx = Package::from_zip(&bytes)
            .and_then(|package| package.stamp(&identity, stamp))
            .and_then(|package| package.crx(&identity))
            .with_context(|| format!("failed to sign {}", zip.display()))?;
        let relative = Path::new(extension.into).join(format!("{id}{CRX_SUFFIX}"));
        let install = root.join(&relative);
        host::write(&install, crx)?;
        host::set_mode(&install, 0o644)?;
        signed.push(Extension {
            id: id.clone(),
            version: stamp.to_string(),
            crx: relative.to_string_lossy().into_owned(),
        });
    }
    Ok(signed)
}
