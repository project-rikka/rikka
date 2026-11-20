mod error;
mod info;
mod store;
mod trash;

pub use error::{Error, Result};
pub use trash::{TrashEntry, can_trash, entries, erase, holds, is_top_level, restore, trash};
