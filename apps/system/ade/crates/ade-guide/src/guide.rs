use std::path::{Path, PathBuf};

use ade_scope::Scope;

use crate::block::block;

pub(crate) const RIKKA: &str = "RIKKA.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guide {
    path: PathBuf,
    scope: Scope,
    text: String,
}

impl Guide {
    pub(crate) fn new(path: &Path, scope: Scope, text: String) -> Guide {
        Guide {
            path: path.to_path_buf(),
            scope,
            text,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn scope(&self) -> Scope {
        self.scope
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn render(&self) -> String {
        block(&self.path, &self.text)
    }
}
