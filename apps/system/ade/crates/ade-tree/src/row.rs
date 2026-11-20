use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    path: PathBuf,
    name: String,
    depth: usize,
    folder: bool,
    open: bool,
}

impl Row {
    pub(crate) fn new(path: PathBuf, name: String, depth: usize, folder: bool, open: bool) -> Row {
        Row {
            path,
            name,
            depth,
            folder,
            open,
        }
    }

    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.folder
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }
}
