#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::Watches;
#[cfg(windows)]
pub use windows::Watches;
