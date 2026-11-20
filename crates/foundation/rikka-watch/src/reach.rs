#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    Uses,
    Entries,
    Contents,
    Tree,
}
