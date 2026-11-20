use std::path::Path;

use anyhow::Result;

use crate::bazel::{self, Profile};
use crate::host;
use crate::release::Version;

const ZIP: &str = "//apps/general:windows_zip";

const VERSION_SETTING: &str = "--//bazel/windows:version";

pub(crate) fn windows(workspace: &Path, version: Version, out: &Path) -> Result<()> {
    let settings = [format!("{VERSION_SETTING}={version}")];
    let built = bazel::build_with(workspace, Profile::Windows, &settings, [ZIP])?;
    let archive = out.join(format!("rikka-{version}-windows-x86_64.zip"));
    host::copy(built.only(ZIP)?, &archive)?;
    println!("{}", archive.display());
    Ok(())
}
