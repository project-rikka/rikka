pub(crate) const LIMIT: usize = 16 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct ClipboardAccess {
    reading: bool,
    writing: bool,
    limit: Option<usize>,
}

impl Default for ClipboardAccess {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardAccess {
    pub fn new() -> Self {
        Self {
            reading: false,
            writing: true,
            limit: Some(LIMIT),
        }
    }

    pub fn reading(mut self, allowed: bool) -> Self {
        self.reading = allowed;
        self
    }

    pub fn writing(mut self, allowed: bool) -> Self {
        self.writing = allowed;
        self
    }

    pub fn limit(mut self, bytes: Option<usize>) -> Self {
        self.limit = bytes;
        self
    }

    pub(crate) fn reads(self) -> bool {
        self.reading
    }

    pub(crate) fn writes(self) -> bool {
        self.writing
    }

    pub(crate) fn limit_bytes(self) -> Option<usize> {
        self.limit
    }
}
