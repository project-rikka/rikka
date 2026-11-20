use rikka_wayland::Scale;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SurfaceSize {
    pub(crate) logical: (u32, u32),
    pub(crate) scale: Scale,
}
