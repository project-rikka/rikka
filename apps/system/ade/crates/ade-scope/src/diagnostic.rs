use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    path: PathBuf,
    message: String,
    loaded: bool,
}

impl Diagnostic {
    #[must_use]
    pub fn skipped(path: &Path, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            path: path.to_path_buf(),
            message: message.into(),
            loaded: false,
        }
    }

    #[must_use]
    pub fn warning(path: &Path, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            path: path.to_path_buf(),
            message: message.into(),
            loaded: true,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}
