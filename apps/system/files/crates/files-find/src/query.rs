use files_core::Location;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub name: String,
    pub contents: String,
    pub options: Options,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub regex: bool,
    pub case_sensitive: bool,
    pub hidden: bool,
    pub respect_ignore_files: bool,
    pub limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            regex: false,
            case_sensitive: false,
            hidden: false,
            respect_ignore_files: true,
            limit: 5_000,
        }
    }
}

impl Query {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.trim().is_empty() && self.contents.trim().is_empty()
    }

    #[must_use]
    pub fn reads_contents(&self) -> bool {
        !self.contents.trim().is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub at: Location,
    pub line: Option<String>,
    pub line_number: Option<u64>,
}
