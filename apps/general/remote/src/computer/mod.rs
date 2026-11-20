#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::name;
#[cfg(windows)]
pub(crate) use windows::name;
