use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Region};
use rikka_compositor_protocols::ext_background_effect_manager_v1::{
    self as manager, Capability, ExtBackgroundEffectManagerV1,
};
use rikka_compositor_protocols::ext_background_effect_surface_v1::{
    self as effect, ExtBackgroundEffectSurfaceV1,
};
use rikka_compositor_surface::SurfaceId;

use super::host::BackgroundEffectHost;
use super::region::BlurRegion;

pub(super) fn bound<S: BackgroundEffectHost>(
    _state: &mut S,
    manager: ExtBackgroundEffectManagerV1,
) {
    manager.capabilities(Capability::BLUR);
}

pub(super) fn manager<S: BackgroundEffectHost>(
    state: &mut S,
    manager: ExtBackgroundEffectManagerV1,
    request: manager::Request,
) {
    let manager::Request::GetBackgroundEffect { id, surface } = request else {
        return;
    };
    let Some(surface) = state.surfaces().lookup(&surface) else {
        return;
    };
    let with_effect = &mut state.background_effects().with_effect;
    if with_effect.contains_key(surface) {
        manager.post_error(manager::Error::BackgroundEffectExists);
        return;
    }
    with_effect.insert(surface, ());
    id.set_key(surface);
}

pub(super) fn request<S: BackgroundEffectHost>(
    state: &mut S,
    effect: ExtBackgroundEffectSurfaceV1,
    request: effect::Request,
) {
    let effect::Request::SetBlurRegion { region } = request else {
        return;
    };
    let Some(surface) = effect.key::<SurfaceId>() else {
        effect.post_error(effect::Error::SurfaceDestroyed);
        return;
    };
    let region = region
        .as_ref()
        .map(|region| state.surfaces().region(region));
    if !stage(state, surface, region) {
        effect.post_error(effect::Error::SurfaceDestroyed);
    }
}

pub(super) fn destroyed<S: BackgroundEffectHost>(
    state: &mut S,
    effect: ExtBackgroundEffectSurfaceV1,
) {
    let Some(surface) = effect.key::<SurfaceId>() else {
        return;
    };
    state.background_effects().with_effect.remove(surface);
    stage(state, surface, None);
}

fn stage<S: BackgroundEffectHost>(
    state: &mut S,
    surface: SurfaceId,
    region: Option<Rc<Region<Logical>>>,
) -> bool {
    let surfaces = state.surfaces();
    if surfaces.get(surface).is_none() {
        return false;
    }
    surfaces.stage::<S, _>(surface, BlurRegion(region));
    true
}
