mod error;
mod open;
mod program;
#[cfg(unix)]
mod reaper;
#[cfg(windows)]
mod shell;
mod spawn;
mod uri;

pub use error::{Error, Result};
#[cfg(unix)]
pub use open::opener;
pub use open::{open, reveal};
pub use program::{find, is_executable, is_runnable, path_dirs};
pub use spawn::spawn_detached;
pub use uri::{decode_path, encode_path, path_to_uri, uri_to_path};
