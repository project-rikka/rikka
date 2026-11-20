use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathMatch {
    path: String,
    matched: Vec<Range<usize>>,
}

impl PathMatch {
    #[must_use]
    pub fn new(path: impl Into<String>) -> PathMatch {
        PathMatch {
            path: path.into(),
            matched: Vec::new(),
        }
    }

    pub(crate) fn picked(path: &str, positions: &[usize]) -> PathMatch {
        PathMatch {
            path: path.to_owned(),
            matched: rikka_fuzzy::byte_ranges(path, positions),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.path.ends_with('/')
    }

    #[must_use]
    pub fn leaf(&self) -> &str {
        &self.path[self.leaf_start()..]
    }

    #[must_use]
    pub fn leaf_start(&self) -> usize {
        let end = self.path.strip_suffix('/').unwrap_or(&self.path);
        end.rfind('/').map_or(0, |at| at + 1)
    }

    #[must_use]
    pub fn under(&self) -> &str {
        &self.path[..self.leaf_start()]
    }

    #[must_use]
    pub fn matched(&self) -> &[Range<usize>] {
        &self.matched
    }
}
