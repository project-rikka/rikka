#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::set_modified;
#[cfg(windows)]
pub use windows::set_modified;
