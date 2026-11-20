mod stores;

pub(crate) use stores::{Elsewhere, Gate};

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use rikka_file_operations::{Conflict, Entry, Local, Operation, Outcome, Task};

pub(crate) struct Sandbox(tempfile::TempDir);

impl Sandbox {
    pub(crate) fn new() -> Self {
        Self(tempfile::tempdir().expect("a folder"))
    }

    pub(crate) fn with_trash() -> Self {
        static DATA_HOME: OnceLock<PathBuf> = OnceLock::new();
        DATA_HOME.get_or_init(|| {
            let home = tempfile::tempdir().expect("a folder").keep();
            unsafe { std::env::set_var("XDG_DATA_HOME", &home) };
            home
        });
        Self::new()
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }

    pub(crate) fn at(&self, relative: &str) -> PathBuf {
        self.0.path().join(relative)
    }

    pub(crate) fn dir(&self, relative: &str) -> PathBuf {
        let path = self.at(relative);
        std::fs::create_dir_all(&path).expect("a folder");
        path
    }

    pub(crate) fn file(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let path = self.at(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::write(&path, contents).expect("a file");
        path
    }

    pub(crate) fn names(&self, relative: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.at(relative))
            .expect("the folder")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

pub(crate) fn run(task: Task<PathBuf>) -> Operation<Entry> {
    rikka_file_operations::run(Arc::new(Local), task)
}

pub(crate) fn finish(task: Task<PathBuf>) -> Outcome<PathBuf> {
    run(task).wait()
}

pub(crate) fn wait_for_conflict(operation: &Operation<Entry>) -> Conflict<Entry> {
    operation
        .wait_until(|snapshot| snapshot.question.is_some())
        .question
        .expect("operation finished without a conflict")
}

pub(crate) fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|_| panic!("{} missing", path.display()))
}
