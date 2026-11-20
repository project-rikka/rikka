#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentType {
    #[default]
    None,
    Photo,
    Video,
    Game,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TearingHint {
    #[default]
    Vsync,
    Async,
}
