#[cfg(unix)]
mod bundled;
#[cfg(windows)]
mod mojang;

use std::path::{Path, PathBuf};

use launcher_meta::{JavaVersion, Platform};

use crate::error::{Error, Result};
use crate::fetch::{Fetcher, Report, Verify};
use crate::store::Store;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Java {
    pub executable: PathBuf,
    pub library_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum JavaChoice {
    #[default]
    Managed,
    Custom(PathBuf),
}

fn release_major(home: &Path) -> Option<u32> {
    let release = std::fs::read_to_string(home.join("release")).ok()?;
    let version = release.lines().find_map(|line| {
        line.strip_prefix("JAVA_VERSION=")
            .map(|value| value.trim().trim_matches('"').to_owned())
    })?;
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    match first {
        1 => parts.next()?.parse().ok(),
        major => Some(major),
    }
}

pub(crate) fn checked(
    executable: PathBuf,
    required: u32,
    library_path: Option<PathBuf>,
) -> Result<Java> {
    let home = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Error::NotJava(executable.clone()))?;
    if !executable.is_file() {
        return Err(Error::NotJava(executable));
    }
    let found = release_major(home).ok_or_else(|| Error::NotJava(executable.clone()))?;
    if found < required {
        return Err(Error::OldJava {
            path: executable,
            required,
            found,
        });
    }
    Ok(Java {
        executable,
        library_path,
    })
}

pub(crate) async fn resolve(
    choice: &JavaChoice,
    fetcher: &Fetcher,
    store: &Store,
    platform: Platform,
    version: &JavaVersion,
    verify: Verify,
    report: &Report,
) -> Result<Java> {
    match choice {
        JavaChoice::Custom(executable) => checked(
            executable.clone(),
            version.major_version,
            native_libraries(),
        ),
        JavaChoice::Managed => managed(fetcher, store, platform, version, verify, report).await,
    }
}

#[cfg(unix)]
fn native_libraries() -> Option<PathBuf> {
    bundled::libraries()
}

#[cfg(windows)]
fn native_libraries() -> Option<PathBuf> {
    None
}

#[cfg(unix)]
async fn managed(
    _fetcher: &Fetcher,
    _store: &Store,
    _platform: Platform,
    version: &JavaVersion,
    _verify: Verify,
    _report: &Report,
) -> Result<Java> {
    bundled::java(version.major_version)
}

#[cfg(windows)]
async fn managed(
    fetcher: &Fetcher,
    store: &Store,
    platform: Platform,
    version: &JavaVersion,
    verify: Verify,
    report: &Report,
) -> Result<Java> {
    mojang::install(fetcher, store, platform, version, verify, report).await
}
