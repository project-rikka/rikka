use std::path::PathBuf;

use rikka_dialog::{Chooser, ChooserResponse};
use rikka_dirs::{UserDir, UserDirs};

#[must_use]
pub(crate) fn choose(title: String, from: Option<String>) -> Option<String> {
    let start = from
        .map(PathBuf::from)
        .filter(|folder| folder.is_dir())
        .or_else(|| UserDirs::load().path(UserDir::Pictures));
    let chooser = Chooser::open(title).directory(true);
    let chooser = match start {
        Some(start) => chooser.current_folder(start),
        None => chooser,
    };
    match chooser.run() {
        Ok(ChooserResponse::Files(folders)) => folders
            .into_iter()
            .next()
            .map(|folder| folder.to_string_lossy().into_owned()),
        Ok(ChooserResponse::Cancelled) => None,
        Err(error) => {
            tracing::warn!("failed to choose photo folder: {error}");
            None
        }
    }
}
