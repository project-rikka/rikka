use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::edit::{Action, Applied, Edit};
use crate::error::Result;
use crate::file;
use crate::project::Project;

const REMEMBERED: usize = 500;

#[derive(Clone, Debug)]
struct Entry {
    action: Action,
    project: Arc<Project>,
    revision: u64,
}

#[derive(Clone, Debug)]
pub struct Document {
    project: Arc<Project>,
    revision: u64,
    counted: u64,
    past: Vec<Entry>,
    future: Vec<Entry>,
    saved: Option<u64>,
    path: Option<PathBuf>,
}

impl Default for Document {
    fn default() -> Document {
        Document::new()
    }
}

impl Document {
    #[must_use]
    pub fn new() -> Document {
        Document::holding(Project::new(), None)
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Document> {
        let path = path.into();
        let project = file::read(&path)?;
        Ok(Document::holding(project, Some(path)))
    }

    fn holding(project: Project, path: Option<PathBuf>) -> Document {
        Document {
            project: Arc::new(project),
            revision: 0,
            counted: 0,
            past: Vec::new(),
            future: Vec::new(),
            saved: Some(0),
            path,
        }
    }

    #[must_use]
    pub fn project(&self) -> &Arc<Project> {
        &self.project
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.saved != Some(self.revision)
    }

    pub fn apply(&mut self, edit: &Edit) -> Result<Applied> {
        let (project, applied) = self.project.apply(edit)?;
        if project == *self.project {
            return Ok(applied);
        }
        self.counted += 1;
        let before = Entry {
            action: edit.action(),
            project: std::mem::replace(&mut self.project, Arc::new(project)),
            revision: std::mem::replace(&mut self.revision, self.counted),
        };
        self.past.push(before);
        if self.past.len() > REMEMBERED {
            self.past.remove(0);
        }
        self.future.clear();
        Ok(applied)
    }

    pub fn amend(&mut self, edit: &Edit) -> Result<Applied> {
        if self.past.is_empty() {
            return self.apply(edit);
        }
        let (project, applied) = self.project.apply(edit)?;
        if project == *self.project {
            return Ok(applied);
        }
        self.counted += 1;
        self.project = Arc::new(project);
        self.revision = self.counted;
        self.future.clear();
        Ok(applied)
    }

    #[must_use]
    pub fn undoable(&self) -> Option<Action> {
        self.past.last().map(|entry| entry.action)
    }

    #[must_use]
    pub fn redoable(&self) -> Option<Action> {
        self.future.last().map(|entry| entry.action)
    }

    pub fn undo(&mut self) -> Option<Action> {
        let entry = self.past.pop()?;
        let action = entry.action;
        let after = self.exchange(entry);
        self.future.push(after);
        Some(action)
    }

    pub fn redo(&mut self) -> Option<Action> {
        let entry = self.future.pop()?;
        let action = entry.action;
        let before = self.exchange(entry);
        self.past.push(before);
        Some(action)
    }

    fn exchange(&mut self, entry: Entry) -> Entry {
        Entry {
            action: entry.action,
            project: std::mem::replace(&mut self.project, entry.project),
            revision: std::mem::replace(&mut self.revision, entry.revision),
        }
    }

    pub fn saved(&mut self, path: PathBuf, revision: u64) {
        self.path = Some(path);
        self.saved = Some(revision);
    }
}
