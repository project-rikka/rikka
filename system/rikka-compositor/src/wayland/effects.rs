use rikka_compositor_output::{BackgroundEffectHost, BackgroundEffects};
use rikka_compositor_surface::SurfaceId;

use crate::state::Compositor;

impl BackgroundEffectHost for Compositor {
    fn background_effects(&mut self) -> &mut BackgroundEffects {
        &mut self.effects
    }

    fn blur_region_committed(&mut self, surface: SurfaceId) {
        let root = self.surfaces.root(surface);
        let showing: Vec<_> = self.scenes.showing(root).collect();
        for output in showing {
            self.frames.request(output);
        }
    }
}
