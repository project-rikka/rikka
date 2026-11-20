use crate::editing::{EditOutcome, EditRequest};
use crate::error::Result;
use crate::files::{Files, WriteOutcome};
use crate::history::History;

#[derive(Clone, Copy, Debug)]
pub struct RecordedFiles<'a> {
    files: &'a Files,
    history: &'a History,
}

impl<'a> RecordedFiles<'a> {
    pub(crate) fn new(files: &'a Files, history: &'a History) -> RecordedFiles<'a> {
        RecordedFiles { files, history }
    }

    pub fn write(&self, path: &str, content: &str) -> Result<WriteOutcome> {
        self.files.write_recorded(path, content, Some(self.history))
    }

    pub fn edit(&self, request: &EditRequest<'_>) -> Result<EditOutcome> {
        self.files.edit_recorded(request, Some(self.history))
    }
}
