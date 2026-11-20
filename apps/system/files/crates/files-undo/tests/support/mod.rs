use std::path::{Path, PathBuf};
use std::sync::Arc;

use files_core::Location;
use files_undo::{Direction, History, Step};
use files_vfs::{Outcome, Task, Vfs};
use rikka_mime::Database;

pub(crate) struct Sandbox {
    root: PathBuf,
    vfs: Arc<Vfs>,
}

impl Sandbox {
    pub(crate) fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("files-undo-{name}"));
        std::fs::remove_dir_all(&root).ok();
        let data = root.join("data");
        std::fs::create_dir_all(data.join("Trash")).expect("create trash");
        unsafe { std::env::set_var("XDG_DATA_HOME", &data) };
        Self {
            root,
            vfs: Arc::new(Vfs::new(Arc::new(Database::load()))),
        }
    }

    pub(crate) fn file(&self, relative: &str, contents: &[u8]) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create directory");
        std::fs::write(&path, contents).expect("write file");
    }

    pub(crate) fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub(crate) fn at(&self, relative: &str) -> Location {
        Location::from_path(self.path(relative))
    }

    pub(crate) fn vfs(&self) -> &Vfs {
        &self.vfs
    }

    pub(crate) fn perform(&self, history: &mut History, task: Task) {
        let kind = task.kind();
        let outcome = self.run(task);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        history.record(Step::from_outcome(kind, &outcome).expect("operation is undoable"));
    }

    pub(crate) fn reverse(&self, history: &mut History, direction: Direction) {
        let reversal = history.begin(direction).expect("step to reverse");
        let outcome = self.run(reversal.task());
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        history.finish(reversal, &outcome);
    }

    fn run(&self, task: Task) -> Outcome {
        rikka_file_operations::run(Arc::clone(&self.vfs), task).wait()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

pub(crate) fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|_| panic!("{} missing", path.display()))
}
