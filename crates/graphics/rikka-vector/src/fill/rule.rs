#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}
