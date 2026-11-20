#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::run;
#[cfg(windows)]
pub(crate) use windows::run;
