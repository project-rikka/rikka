use std::path::{Path, PathBuf};

use ade_scope::{Diagnostic, Scope, listing};
use rikka_fs::Replace;

use crate::index;
use crate::memory::Memory;

const MEMORY: &str = "memory";

pub(crate) const INDEX: &str = "MEMORY.md";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Memories {
    folder: Option<PathBuf>,
    memories: Vec<Memory>,
    diagnostics: Vec<Diagnostic>,
}

impl Memories {
    #[must_use]
    pub fn find(project: Option<&Path>) -> Memories {
        let mut catalog = Memories::default();

        let (Some(folder), Some(root)) = (Memories::folder(project), project) else {
            return catalog;
        };

        for note in notes(&folder) {
            catalog.take(&note, root);
        }

        catalog.memories.sort_by(|one, two| {
            (one.about(), one.title(), one.name()).cmp(&(two.about(), two.title(), two.name()))
        });
        catalog.folder = Some(folder);
        catalog
    }

    #[must_use]
    pub fn folder(project: Option<&Path>) -> Option<PathBuf> {
        Some(Scope::Project.rikka(project)?.join(MEMORY))
    }

    #[must_use]
    pub fn index(project: Option<&Path>) -> Option<PathBuf> {
        Some(Memories::folder(project)?.join(INDEX))
    }

    #[must_use]
    pub fn all(&self) -> &[Memory] {
        &self.memories
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn render(&self) -> String {
        let Some(folder) = &self.folder else {
            return String::new();
        };

        format!(
            "<project_memory folder=\"{}\">\n{}\n</project_memory>",
            folder.display(),
            index::render(&self.memories)
        )
    }

    pub fn write_index(&self) -> std::io::Result<bool> {
        let Some(folder) = &self.folder else {
            return Ok(false);
        };
        if !folder.is_dir() {
            return Ok(false);
        }

        let at = folder.join(INDEX);
        let rendered = index::file(&self.memories);
        if std::fs::read_to_string(&at).is_ok_and(|current| current == rendered) {
            return Ok(false);
        }

        Replace::new(at).private().write(rendered)?;
        Ok(true)
    }

    fn take(&mut self, note: &Path, root: &Path) {
        let (memory, notes) = match Memory::read(note, root) {
            Ok(read) => read,
            Err(reason) => {
                return self
                    .diagnostics
                    .push(Diagnostic::skipped(note, reason.to_string()));
            }
        };

        for message in notes {
            self.diagnostics.push(Diagnostic::warning(note, message));
        }
        self.memories.push(memory);
    }
}

fn notes(folder: &Path) -> Vec<PathBuf> {
    let mut notes = listing::markdown_files(folder);
    notes.retain(|path| {
        path.file_name()
            .is_none_or(|name| !name.eq_ignore_ascii_case(INDEX))
    });
    notes
}
