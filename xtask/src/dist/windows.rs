use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::bazel::{self, Profile};
use crate::components::COMPONENTS;
use crate::dist::build::FILE_PREFIX;
use crate::host;
use crate::release::Version;

const VERSION_SETTING: &str = "--//bazel/windows:version";

const SYSTEM: &str = "windows-x86_64";

pub(crate) fn windows(workspace: &Path, version: Version, out: &Path) -> Result<Vec<PathBuf>> {
    let shipped: Vec<(&str, &str)> = COMPONENTS
        .iter()
        .filter_map(|component| component.windows.map(|zip| (component.id, zip)))
        .collect();
    let settings = [format!("{VERSION_SETTING}={version}")];
    let built = bazel::build_with(
        workspace,
        Profile::Windows,
        &settings,
        shipped.iter().map(|&(_, zip)| zip),
    )?;
    let mut archives = Vec::new();
    for (id, zip) in shipped {
        let archive = out.join(format!("{FILE_PREFIX}{id}-{version}-{SYSTEM}.zip"));
        host::copy(built.only(zip)?, &archive)?;
        println!(
            "packed {id} {version} for Windows into {}",
            archive.display()
        );
        archives.push(archive);
    }
    Ok(archives)
}
