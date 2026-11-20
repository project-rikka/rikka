use std::path::PathBuf;

use rikka_widgets::ImageFit;
use shell_kit::appearance;

use crate::library::Library;

mod backdrop;
mod entry;
mod error;
mod kind;
mod library;
mod library_watcher;
mod loader;
mod material;
mod media;
mod shelf;
pub mod stage;
mod thumbnails;

pub use backdrop::{Backdrop, CROSSING, CROSSING_CURVE, Decision, ShownOn};
use library_watcher::LibraryWatcher;
pub use loader::{cover_of, load, shared};
pub use material::MaterialTheme;
pub use media::{Media, Picture};
use shelf::Shelf;

pub const FIT: ImageFit = ImageFit::Cover;

#[must_use]
pub fn file_of(id: &str) -> Option<PathBuf> {
    let entry = Library::session().ok()?.at_id(id)?;
    Some(entry.path().to_path_buf())
}

#[must_use]
pub fn file_of_setting(value: String) -> Option<PathBuf> {
    appearance::wallpaper_id(value).as_deref().and_then(file_of)
}
