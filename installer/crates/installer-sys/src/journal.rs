use std::sync::Mutex;

use crate::command::Command;

#[derive(Debug, Default)]
pub(crate) struct Journal(Mutex<Vec<String>>);

impl Journal {
    pub(crate) fn entries(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|entries| entries.clone())
            .unwrap_or_default()
    }

    pub(crate) fn command(&self, command: &Command) {
        self.record(command.to_string());
    }

    pub(crate) fn write(&self, path: &str, contents: &str, mode: u32) {
        self.record(format!("write {path} ({mode:o}, {} bytes)", contents.len()));
    }

    pub(crate) fn mkdir(&self, path: &str, mode: u32) {
        self.record(format!("mkdir {path} ({mode:o})"));
    }

    fn record(&self, entry: String) {
        if let Ok(mut entries) = self.0.lock() {
            entries.push(entry);
        }
    }
}
