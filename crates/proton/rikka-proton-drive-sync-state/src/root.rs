use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RootId(pub(crate) i64);

impl RootId {
    #[must_use]
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for RootId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Computer,
    MyFiles(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub local_path: PathBuf,
    pub target: Target,
    pub volume: String,
    pub share: String,
    pub remote_root: String,
    pub local_root: u64,
}
