#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub(crate) strings: usize,
    pub(crate) chars: usize,
}

impl Limits {
    #[must_use]
    pub const fn new(strings: usize, chars: usize) -> Limits {
        assert!(
            strings > 0 && chars > 0,
            "request limit must allow at least one string"
        );
        Limits { strings, chars }
    }
}
