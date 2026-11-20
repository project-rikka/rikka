#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ExclusiveZone {
    #[default]
    None,
    Fixed(f32),
    Ignore,
}
