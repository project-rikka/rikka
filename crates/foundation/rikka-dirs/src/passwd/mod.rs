#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{current, current_uid, lookup};
#[cfg(windows)]
pub use windows::current;

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub gecos: String,
    pub home: PathBuf,
}

impl Entry {
    #[must_use]
    pub fn full_name(&self) -> Option<&str> {
        let name = self.gecos.split(',').next().unwrap_or_default().trim();
        (!name.is_empty()).then_some(name)
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        self.full_name().unwrap_or(&self.name)
    }

    #[must_use]
    pub fn initial(&self) -> String {
        self.display_name()
            .chars()
            .next()
            .map(|first| first.to_uppercase().collect())
            .unwrap_or_default()
    }
}
