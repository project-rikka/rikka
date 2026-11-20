#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::{create, open};
#[cfg(windows)]
pub(crate) use windows::{create, open};

#[derive(Debug)]
pub(crate) enum Refusal {
    Link,
    Io(std::io::Error),
}
