#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub old: String,
    pub new: String,
    pub all: bool,
}

impl Replacement {
    #[must_use]
    pub fn new(old: impl Into<String>, new: impl Into<String>) -> Replacement {
        Replacement {
            old: old.into(),
            new: new.into(),
            all: false,
        }
    }

    #[must_use]
    pub fn all(old: impl Into<String>, new: impl Into<String>) -> Replacement {
        Replacement {
            all: true,
            ..Replacement::new(old, new)
        }
    }
}
