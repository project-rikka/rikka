mod error;
mod mutf8;
mod read;
mod tag;
mod write;

pub use error::{Error, Result};
pub use read::read;
pub use tag::{Compound, Kind, List, Tag};
pub use write::write;
