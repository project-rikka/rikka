#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tearing {
    #[default]
    Forbidden,
    Allowed,
}
