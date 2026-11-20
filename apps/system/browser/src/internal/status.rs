#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Status {
    #[default]
    Waiting,
    Ready,
    Failed,
}
