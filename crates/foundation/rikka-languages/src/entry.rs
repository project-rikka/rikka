#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct Entry {
    pub(crate) tag: &'static str,
    pub(crate) english: &'static str,
    pub(crate) native: &'static str,
}

impl Entry {
    pub(crate) const fn new(
        tag: &'static str,
        english: &'static str,
        native: &'static str,
    ) -> Entry {
        Entry {
            tag,
            english,
            native,
        }
    }
}
