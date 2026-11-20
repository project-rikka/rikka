#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::tag;
#[cfg(windows)]
pub(crate) use windows::tag;
