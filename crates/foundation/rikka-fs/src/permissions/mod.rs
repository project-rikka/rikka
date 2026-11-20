#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::apply;
#[cfg(windows)]
pub(crate) use windows::apply;
