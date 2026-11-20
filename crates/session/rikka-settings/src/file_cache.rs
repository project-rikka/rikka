use rikka_sync::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct FileCache(Arc<Mutex<HashMap<PathBuf, Option<String>>>>);

impl FileCache {
    pub(crate) fn read(&self, path: &Path) -> Option<Option<String>> {
        self.documents().get(path).cloned()
    }

    pub(crate) fn write(&self, path: PathBuf, document: Option<String>) {
        self.documents().insert(path, document);
    }

    fn documents(&self) -> rikka_sync::MutexGuard<'_, HashMap<PathBuf, Option<String>>> {
        self.0.lock()
    }
}

impl std::fmt::Debug for FileCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Kept")
            .field("files", &self.documents().len())
            .finish()
    }
}
