use std::path::Path;

use rikka_chat::{Attached, read_mentioned};

use crate::project::chat::AdeChat;

pub(crate) fn mentioned(root: &Path, paths: &[String]) -> Attached {
    read_mentioned::<AdeChat>(root, paths, listed)
}

fn listed(folder: &Path) -> Vec<String> {
    ade_tree::read(folder)
        .iter()
        .map(|entry| match entry.is_folder() {
            true => format!("{}/", entry.name()),
            false => entry.name().to_owned(),
        })
        .collect()
}
