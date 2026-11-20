use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::entries::Entries;
use crate::system;

const IMPORT_MARKER: &str = "DLL Name:";

pub(crate) fn check(objdump: &Path, entries: &Entries) -> Result<()> {
    let shipped: BTreeSet<String> = entries.shipped_libraries().collect();
    let mut strays = Vec::new();
    for binary in entries.binaries() {
        for library in imported(objdump, &binary.source)? {
            if !shipped.contains(&library) && !system::provides(&library) {
                strays.push(format!("{} imports {library}", binary.name));
            }
        }
    }
    if !strays.is_empty() {
        bail!(
            "the package needs libraries that neither Windows nor the package provide:\n  {}",
            strays.join("\n  ")
        );
    }
    Ok(())
}

fn imported(objdump: &Path, binary: &Path) -> Result<BTreeSet<String>> {
    let output = Command::new(objdump)
        .arg("-p")
        .arg(binary)
        .output()
        .with_context(|| format!("failed to run {}", objdump.display()))?;
    if !output.status.success() {
        bail!(
            "{} -p {} failed: {}",
            objdump.display(),
            binary.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let listing = String::from_utf8(output.stdout)
        .with_context(|| format!("{} printed non-UTF-8 text", objdump.display()))?;
    Ok(listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix(IMPORT_MARKER))
        .map(|library| library.trim().to_ascii_lowercase())
        .collect())
}
