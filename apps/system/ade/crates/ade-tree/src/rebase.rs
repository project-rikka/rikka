use std::path::{Path, PathBuf};

#[must_use]
pub fn rebased(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(from).ok()?;
    Some(match rest.as_os_str().is_empty() {
        true => to.to_owned(),
        false => to.join(rest),
    })
}
