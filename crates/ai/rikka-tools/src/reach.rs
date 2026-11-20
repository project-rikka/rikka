#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Reach {
    #[default]
    Working,
    Reading,
}
