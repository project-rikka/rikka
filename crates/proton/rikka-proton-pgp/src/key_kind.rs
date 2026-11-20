#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum KeyKind {
    #[default]
    Curve25519,
    PostQuantum,
}
