mod error;
mod library;
mod show;

pub use error::{Error, Result};
pub use library::{Library, is_wallpaper};
pub use show::show;
