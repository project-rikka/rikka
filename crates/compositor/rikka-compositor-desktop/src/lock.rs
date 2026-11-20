use rikka_compositor_shell::LockSurfaceId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;
use slotmap::SecondaryMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Guard {
    pub(crate) surface: SurfaceId,
    pub(crate) output: OutputId,
}

#[derive(Debug, Default)]
pub(crate) struct Lock {
    pub(crate) locked: bool,
    pub(crate) surfaces: SecondaryMap<LockSurfaceId, Guard>,
}
