#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::{decode, encode};
#[cfg(windows)]
pub(crate) use windows::{decode, encode};
