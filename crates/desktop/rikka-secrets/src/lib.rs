pub mod blocking;
mod entry;
mod error;
#[cfg(unix)]
mod keyring;
pub mod schema;
#[cfg(unix)]
mod secrets;
#[cfg(windows)]
mod windows;

pub use entry::{Attributes, Entry, Found, ItemId, Secret, StoredItem};
pub use error::{Error, Result};
#[cfg(unix)]
pub use keyring::{Keyring, KeyringState, LoginOutcome, PromptId, Status};
#[cfg(unix)]
pub use secrets::{Change, Secrets};
#[cfg(windows)]
pub use windows::Secrets;
