use std::sync::Arc;

use rikka_file_edit::Files;

use crate::file_history::FileHistory;
use crate::path_instructions::PathInstructions;

pub(crate) struct FileAccess {
    pub(crate) files: Arc<Files>,
    pub(crate) history: FileHistory,
    pub(crate) aside: Arc<dyn PathInstructions>,
}
