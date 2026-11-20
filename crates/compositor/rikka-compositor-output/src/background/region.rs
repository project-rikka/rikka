use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Region};
use rikka_compositor_surface::{Cached, SurfaceId, Surfaces};

use super::host::BackgroundEffectHost;

#[derive(Debug)]
pub(super) struct BlurRegion(pub(super) Option<Rc<Region<Logical>>>);

impl<S: BackgroundEffectHost> Cached<S> for BlurRegion {
    fn applied(state: &mut S, surface: SurfaceId) {
        state.blur_region_committed(surface);
    }
}

pub fn blur_region(surfaces: &Surfaces, surface: SurfaceId) -> Option<&Region<Logical>> {
    surfaces
        .cached::<BlurRegion>(surface)?
        .0
        .as_deref()
        .filter(|region| !region.is_empty())
}
