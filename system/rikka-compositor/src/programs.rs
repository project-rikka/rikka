use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) const COMPOSITOR: &str = "rikka-compositor";

pub(crate) const SHELL: &str = "rikka-shell";

pub(crate) fn shell() -> PathBuf {
    bin()
        .map(|bin| bin.join(SHELL))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(SHELL))
}

pub(crate) fn search_path() -> Option<OsString> {
    let bin = bin()?;
    let Some(inherited) = std::env::var_os("PATH").filter(|path| !path.is_empty()) else {
        return Some(bin.into_os_string());
    };
    if std::env::split_paths(&inherited).any(|directory| directory == bin) {
        return None;
    }
    match std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited))) {
        Ok(path) => Some(path),
        Err(error) => {
            tracing::warn!("cannot prepend executable directory to PATH: {error}");
            None
        }
    }
}

pub(crate) fn prefix() -> Option<PathBuf> {
    Some(bin()?.parent()?.to_path_buf())
}

fn bin() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
}
