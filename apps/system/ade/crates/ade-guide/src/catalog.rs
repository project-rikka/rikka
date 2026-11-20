use std::path::{Path, PathBuf};

use ade_scope::{Diagnostic, Scope};

use crate::guide::{Guide, RIKKA};
use crate::reading;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Guides {
    guides: Vec<Guide>,
    diagnostics: Vec<Diagnostic>,
}

impl Guides {
    #[must_use]
    pub fn find(project: Option<&Path>) -> Guides {
        let mut catalog = Guides::default();

        for scope in Scope::GENERAL_FIRST {
            let Some(path) = Guides::file(scope, project) else {
                continue;
            };
            catalog.take(&path, scope);
        }

        catalog
    }

    #[must_use]
    pub fn file(scope: Scope, project: Option<&Path>) -> Option<PathBuf> {
        let at = match scope {
            Scope::User => scope.rikka(project)?,
            Scope::Project => scope.root(project)?,
        };
        Some(at.join(RIKKA))
    }

    #[must_use]
    pub fn all(&self) -> &[Guide] {
        &self.guides
    }

    #[must_use]
    pub fn get(&self, scope: Scope) -> Option<&Guide> {
        self.guides.iter().find(|guide| guide.scope() == scope)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.guides.is_empty()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn render(&self) -> String {
        self.guides
            .iter()
            .map(Guide::render)
            .collect::<Vec<String>>()
            .join("\n\n")
    }

    fn take(&mut self, path: &Path, scope: Scope) {
        match reading::read(path) {
            Err(message) => self.diagnostics.push(Diagnostic::skipped(path, message)),
            Ok(None) => {}
            Ok(Some(read)) => {
                if let Some(message) = read.note {
                    self.diagnostics.push(Diagnostic::warning(path, message));
                }
                self.guides.push(Guide::new(path, scope, read.text));
            }
        }
    }
}
