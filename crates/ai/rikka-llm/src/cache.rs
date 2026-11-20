#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cache {
    #[default]
    Off,
    Prefix,
    Conversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ttl {
    #[default]
    Short,
    Long,
}
