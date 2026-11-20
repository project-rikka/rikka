mod error;
mod module;
mod modules;
mod packages;

pub use error::{Error, Result};
pub use module::{DIRECTORY, FLAKE, HEADER, Module};
pub use modules::Modules;
pub use packages::Packages;
