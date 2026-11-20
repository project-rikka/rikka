#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Job {
    Apply,
    Update,
    Rollback(u32),
}
