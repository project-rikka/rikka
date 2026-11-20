#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Locating,
    Downloading { received: u64, total: Option<u64> },
    Indexing,
}
