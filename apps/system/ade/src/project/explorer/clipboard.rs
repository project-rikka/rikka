use std::path::{Path, PathBuf};

use rikka_clipboard::Offer;
use rikka_open::{path_to_uri, uri_to_path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClipboardFiles {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) cut: bool,
}

pub(crate) fn copy_files(paths: &[PathBuf], cut: bool) {
    if paths.is_empty() {
        return;
    }
    let uris: Vec<String> = paths.iter().map(|path| path_to_uri(path)).collect();
    let offer = Offer::files(&uris, cut).with_text(paths_text(paths, None));
    if let Err(error) = rikka_ui::clipboard::publish(offer) {
        tracing::warn!(%error, "failed to copy files to clipboard");
    }
}

pub(crate) fn take_files() -> Option<ClipboardFiles> {
    let files = rikka_ui::clipboard::files()?;
    if files.cut
        && let Err(error) = rikka_ui::clipboard::publish(Offer::new())
    {
        tracing::warn!(%error, "failed to clear clipboard after paste");
    }
    let paths = paths_of(&files.uris);
    (!paths.is_empty()).then_some(ClipboardFiles {
        paths,
        cut: files.cut,
    })
}

pub(crate) fn paths_of(uris: &[String]) -> Vec<PathBuf> {
    uris.iter().filter_map(|uri| uri_to_path(uri)).collect()
}

pub(crate) fn paths_text(paths: &[PathBuf], from: Option<&Path>) -> String {
    paths
        .iter()
        .map(
            |path| match from.and_then(|root| path.strip_prefix(root).ok()) {
                Some(relative) => relative.display().to_string(),
                None => path.display().to_string(),
            },
        )
        .collect::<Vec<_>>()
        .join("\n")
}
