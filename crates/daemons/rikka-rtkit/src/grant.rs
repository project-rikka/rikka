#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grantor {
    Kernel,
    RealtimeKit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant<L> {
    pub level: L,
    pub grantor: Grantor,
}
