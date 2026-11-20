#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitOperation {
    Committing,
    Switching,
}
