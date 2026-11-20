#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Buffering {
    #[default]
    Triple,
    Double,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceConfig {
    pub transparent: bool,
    pub buffering: Buffering,
}

#[derive(Debug)]
pub enum Acquired<T> {
    Image(T),
    Stale,
    NotReady,
}
