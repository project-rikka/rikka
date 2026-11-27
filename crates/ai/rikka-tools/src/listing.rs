use std::path::PathBuf;

use rikka_file_edit::Listing;

use crate::look::{Inspection, look};
use crate::words::exact_segments;

pub(crate) fn listing(command: &str) -> Option<Listing> {
    if look(command)?.inspection() == Inspection::List {
        return None;
    }

    let mut listing = Listing::new();
    let mut folder = Some(PathBuf::new());
    for words in exact_segments(command)? {
        let (name, arguments) = words.split_first()?;
        if name == "cd" {
            folder = match arguments.first() {
                Some(target) if !target.starts_with('-') => {
                    folder.map(|folder| folder.join(target))
                }
                _ => None,
            };
            if let Some(entered) = folder.as_ref().and_then(|folder| folder.to_str()) {
                listing = listing.folder(entered);
            }
            continue;
        }
        let Some(folder) = &folder else {
            continue;
        };
        for word in arguments.iter().filter(|word| !word.starts_with('-')) {
            if let Some(named) = folder.join(word).to_str() {
                listing = listing.file(named);
            }
        }
    }
    Some(listing)
}
