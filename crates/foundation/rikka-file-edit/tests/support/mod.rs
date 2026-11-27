use rikka_file_edit::{EditOutcome, EditRequest, Files, Replacement};

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

    pub(crate) fn apply(
        &self,
        path: &str,
        replacements: Vec<Replacement>,
    ) -> Result<EditOutcome, String> {
        self.files
            .edit(&EditRequest { path, replacements })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn edit(
        &self,
        path: &str,
        replacements: Vec<Replacement>,
    ) -> Result<String, String> {
        self.apply(path, replacements)
            .map(|outcome| outcome.summary)
    }
}
