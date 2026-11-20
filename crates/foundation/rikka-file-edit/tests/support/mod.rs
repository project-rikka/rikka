use rikka_file_edit::{EditOutcome, EditRequest, Files, Op};

pub(crate) struct Project {
    pub(crate) folder: tempfile::TempDir,
    pub(crate) files: Files,
}

impl Project {
    pub(crate) fn open() -> Project {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let files = Files::open(folder.path()).expect("it is a folder");
        Project { folder, files }
    }

    pub(crate) fn write_file(&self, path: &str, text: &str) {
        let at = self.folder.path().join(path);
        if let Some(folder) = at.parent() {
            std::fs::create_dir_all(folder).expect("the folder is made");
        }
        std::fs::write(at, text).expect("it is written");
    }

    pub(crate) fn read_file(&self, path: &str) -> String {
        std::fs::read_to_string(self.folder.path().join(path)).expect("it is there")
    }

    pub(crate) fn apply(&self, path: &str, ops: Vec<Op>) -> Result<EditOutcome, String> {
        self.files
            .edit(&EditRequest { path, ops })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn edit(&self, path: &str, ops: Vec<Op>) -> Result<String, String> {
        self.apply(path, ops).map(|outcome| outcome.summary)
    }
}

pub(crate) fn replace_op(start: usize, end: usize, text: &str) -> Op {
    Op::Replace {
        start,
        end,
        text: text.to_owned(),
    }
}

pub(crate) fn insert_op(line: usize, text: &str) -> Op {
    Op::InsertBefore {
        line,
        text: text.to_owned(),
    }
}

pub(crate) fn delete_op(start: usize, end: usize) -> Op {
    Op::Delete { start, end }
}
