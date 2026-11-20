#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Flip {
    #[default]
    Vsync,
    Async,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Pacing {
    #[default]
    Balanced,
    LowLatency,
    Steady,
    Immediate,
}
