#[cfg(unix)]
mod session;
#[cfg(windows)]
mod system;

#[cfg(unix)]
pub(super) use session::{Following, follow};
#[cfg(windows)]
pub(super) use system::{Following, follow};
