use rikka_sync::Mutex;
use std::sync::Arc;

use rikka_file_edit::{EditOutcome, EditRequest, Files, History, WriteOutcome};

#[derive(Clone, Debug, Default)]
pub(crate) struct FileHistory {
    history: Arc<Mutex<Option<History>>>,
}

impl FileHistory {
    pub(crate) fn keep_in(&self, history: History) {
        *self.slot() = Some(history);
    }

    pub(crate) fn history(&self) -> Option<History> {
        self.slot().clone()
    }

    pub(crate) fn write(
        &self,
        files: &Files,
        path: &str,
        content: &str,
    ) -> rikka_file_edit::Result<WriteOutcome> {
        match self.history() {
            Some(history) => files.with_history(&history).write(path, content),
            None => files.write(path, content),
        }
    }

    pub(crate) fn edit(
        &self,
        files: &Files,
        request: &EditRequest<'_>,
    ) -> rikka_file_edit::Result<EditOutcome> {
        match self.history() {
            Some(history) => files.with_history(&history).edit(request),
            None => files.edit(request),
        }
    }

    fn slot(&self) -> rikka_sync::MutexGuard<'_, Option<History>> {
        self.history.lock()
    }
}
