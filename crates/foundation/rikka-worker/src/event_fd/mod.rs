#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{EventFd, EventNotifier, event_fd, wait_readable};
#[cfg(windows)]
pub use windows::{EventFd, EventNotifier, event_fd};
