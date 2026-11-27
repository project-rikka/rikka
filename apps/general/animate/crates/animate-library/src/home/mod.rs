#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::root;
#[cfg(windows)]
pub(crate) use windows::root;
