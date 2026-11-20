#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyboardMode {
    #[default]
    None,
    OnDemand,
    Exclusive,
}
