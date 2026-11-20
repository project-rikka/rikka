#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub(crate) name: String,
    pub(crate) mime_types: Vec<String>,
    pub(crate) patterns: Vec<String>,
}

impl Filter {
    #[must_use]
    pub fn named(name: impl Into<String>) -> Filter {
        Filter {
            name: name.into(),
            mime_types: Vec::new(),
            patterns: Vec::new(),
        }
    }

    #[must_use]
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Filter {
        self.mime_types.push(mime_type.into());
        self
    }

    #[must_use]
    pub fn pattern(mut self, glob: impl Into<String>) -> Filter {
        self.patterns.push(glob.into());
        self
    }
}
