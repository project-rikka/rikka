#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    name: String,
    folder: bool,
}

impl Entry {
    #[must_use]
    pub fn new(name: impl Into<String>, folder: bool) -> Entry {
        Entry {
            name: name.into(),
            folder,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.folder
    }
}
