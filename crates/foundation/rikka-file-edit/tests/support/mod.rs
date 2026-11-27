use rikka_file_edit::{Anchor, Edit, EditOutcome, EditRequest, Files, Listing};

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

    pub(crate) fn listing(&self, path: &str) -> String {
        let numbered: String = self
            .read_file(path)
            .split_inclusive('\n')
            .enumerate()
            .map(|(index, line)| format!("{:>6}\t{line}", index + 1))
            .collect();
        self.files.anchor(&Listing::new().file(path), &numbered)
    }

    pub(crate) fn at(&self, path: &str, line: usize) -> Anchor {
        let listing = self.listing(path);
        let row = listing.lines().nth(line - 1).expect("the line is listed");
        let (anchor, _) = row.trim_start().split_once('\t').expect("a numbered row");
        anchor.parse().expect("the row is anchored")
    }

    pub(crate) fn lines(&self, path: &str, first: usize, last: usize, text: &str) -> Edit {
        Edit::Replace {
            first: self.at(path, first),
            last: self.at(path, last),
            text: text.to_owned(),
        }
    }

    pub(crate) fn after(&self, path: &str, line: usize, text: &str) -> Edit {
        Edit::InsertAfter {
            anchor: self.at(path, line),
            text: text.to_owned(),
        }
    }

    pub(crate) fn before(&self, path: &str, line: usize, text: &str) -> Edit {
        Edit::InsertBefore {
            anchor: self.at(path, line),
            text: text.to_owned(),
        }
    }

    pub(crate) fn apply(&self, path: &str, edits: Vec<Edit>) -> Result<EditOutcome, String> {
        self.files
            .edit(&EditRequest { path, edits })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn edit(&self, path: &str, edits: Vec<Edit>) -> Result<String, String> {
        self.apply(path, edits).map(|outcome| outcome.summary)
    }
}

pub(crate) fn anchor(text: &str) -> Anchor {
    text.parse().expect("a valid anchor")
}
