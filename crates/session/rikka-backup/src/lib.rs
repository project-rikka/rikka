mod bundle;
mod bytes;
mod configured;
mod credential;
mod error;
mod file;
mod module_name;
mod origin;
mod preference;
mod repository;

pub use bundle::{Bundle, PORTABLE};
pub use configured::Configured;
pub use credential::Credential;
pub use error::{Error, Result};
pub use file::{Base, File};
pub use origin::Origin;
pub use preference::Preference;
pub use repository::{BRANCH, checked_url, clone_arguments};

pub const EXTENSION: &str = "rikka-backup";
pub const PENDING: &str = "restore.rikka-backup";
pub const HOME_STATE: &str = ".local/state/rikka";
