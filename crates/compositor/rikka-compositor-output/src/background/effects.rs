use rikka_compositor_surface::SurfaceId;
use slotmap::SecondaryMap;

#[derive(Debug, Default)]
pub struct BackgroundEffects {
    pub(super) with_effect: SecondaryMap<SurfaceId, ()>,
}
