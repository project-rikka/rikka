use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    name: String,
    about: String,
    path: PathBuf,
}

impl Place {
    #[must_use]
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Place {
        Place {
            name: name.into(),
            about: String::new(),
            path: path.into(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, about: impl Into<String>) -> Place {
        self.about = about.into();
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.about
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
