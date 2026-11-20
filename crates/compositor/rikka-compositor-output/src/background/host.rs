use rikka_compositor_surface::{SurfaceHost, SurfaceId};

use super::effects::BackgroundEffects;

pub trait BackgroundEffectHost: SurfaceHost {
    fn background_effects(&mut self) -> &mut BackgroundEffects;

    fn blur_region_committed(&mut self, _surface: SurfaceId) {}
}
