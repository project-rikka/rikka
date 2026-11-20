mod base;
mod error;
#[cfg(windows)]
mod known;
mod paths;
mod user_dirs;

pub mod passwd;

pub use base::{
    cache_home, config_dirs, config_home, data_dirs, data_home, home, runtime_dir, state_home,
};
pub use error::{Error, Result};
pub use paths::{Paths, ensure};
pub use user_dirs::{UserDir, UserDirs};
