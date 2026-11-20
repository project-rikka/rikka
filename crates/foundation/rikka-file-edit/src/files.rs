use rikka_sync::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::text::Text;

use crate::error::{Error, Result};
use crate::history::History;
use crate::recorded_files::RecordedFiles;
use crate::workspace::Workspace;
use crate::writer::{self, Precondition};

#[derive(Debug)]
pub struct Files {
    workspace: Workspace,
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

#[derive(Debug)]
pub struct WriteOutcome {
    pub path: PathBuf,
    pub relative_path: String,
    pub lines: usize,
    pub created: bool,
}

impl Files {
    pub fn open(project: &Path) -> Result<Files> {
        Ok(Files {
            workspace: Workspace::open(project)?,
            locks: Mutex::new(HashMap::new()),
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.workspace.root()
    }

    #[must_use]
    pub fn resolve(&self, path: &str) -> PathBuf {
        self.workspace.resolve(path)
    }

    pub(crate) fn resolve_inside(&self, path: &str) -> Result<PathBuf> {
        self.workspace.resolve_inside(path)
    }

    #[must_use]
    pub fn relative_path(&self, path: &Path) -> String {
        self.workspace.relative_path(path)
    }

    pub fn write(&self, path: &str, content: &str) -> Result<WriteOutcome> {
        self.write_recorded(path, content, None)
    }

    #[must_use]
    pub fn with_history<'a>(&'a self, history: &'a History) -> RecordedFiles<'a> {
        RecordedFiles::new(self, history)
    }

    pub(crate) fn write_recorded(
        &self,
        path: &str,
        content: &str,
        history: Option<&History>,
    ) -> Result<WriteOutcome> {
        let path = self.resolve_inside(path)?;
        let relative_path = self.relative_path(&path);

        if path.is_dir() {
            return Err(Error::Io {
                path: relative_path,
                reason: "it is a folder rather than a file".to_owned(),
            });
        }

        let lock = self.path_lock(&path);
        let _guard = lock.lock();

        let previous = self.read_bytes(&path, &relative_path)?;
        let text = match previous
            .as_deref()
            .and_then(|bytes| Text::parse(bytes).ok())
        {
            Some(file) => file.with_content(content),
            None => Text::from_content(content),
        };

        if let Some(history) = history {
            history.record(&path, previous.as_deref())?;
        }
        writer::write(&path, &text.render(), &Precondition::Any)?;

        Ok(WriteOutcome {
            path,
            relative_path,
            lines: text.line_count(),
            created: previous.is_none(),
        })
    }

    pub(crate) fn read_text(&self, path: &Path, relative_path: &str) -> Result<(Vec<u8>, Text)> {
        let bytes = self
            .read_bytes(path, relative_path)?
            .ok_or_else(|| Error::NotFound(relative_path.to_owned()))?;

        let text = Text::parse(&bytes).map_err(|_| Error::NotText(relative_path.to_owned()))?;
        Ok((bytes, text))
    }

    fn read_bytes(&self, path: &Path, relative_path: &str) -> Result<Option<Vec<u8>>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Io {
                path: relative_path.to_owned(),
                reason: error.to_string(),
            }),
        }
    }

    pub(crate) fn path_lock(&self, path: &Path) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock();
        Arc::clone(locks.entry(path.to_owned()).or_default())
    }
}
