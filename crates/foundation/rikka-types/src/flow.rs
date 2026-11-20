#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    #[default]
    Continue,
    Cancel,
}

impl Flow {
    #[must_use]
    pub fn is_cancel(self) -> bool {
        self == Flow::Cancel
    }
}

impl From<bool> for Flow {
    fn from(keep_going: bool) -> Flow {
        if keep_going {
            Flow::Continue
        } else {
            Flow::Cancel
        }
    }
}
