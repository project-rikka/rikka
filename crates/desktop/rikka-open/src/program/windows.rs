use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(super) const SEPARATORS: [char; 2] = ['/', '\\'];

const DEFAULT_EXTENSIONS: &str = ".COM;.EXE;.BAT;.CMD";

#[must_use]
pub fn is_executable(path: &Path) -> bool {
    let runnable = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions()
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        });
    runnable && std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

pub(super) fn runnable_in(directory: &Path, program: &str) -> Option<PathBuf> {
    let named = directory.join(program);
    if is_executable(&named) {
        return Some(named);
    }
    extensions().into_iter().find_map(|extension| {
        let mut candidate = OsString::from(named.as_os_str());
        candidate.push(".");
        candidate.push(extension);
        let candidate = PathBuf::from(candidate);
        is_executable(&candidate).then_some(candidate)
    })
}

fn extensions() -> Vec<String> {
    let listed = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_EXTENSIONS.to_owned());
    listed
        .split(';')
        .map(|extension| extension.trim().trim_start_matches('.'))
        .filter(|extension| !extension.is_empty())
        .map(str::to_owned)
        .collect()
}
