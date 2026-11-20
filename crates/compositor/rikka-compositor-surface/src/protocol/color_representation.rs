use rikka_compositor_protocols::wp_color_representation_manager_v1::{
    self, WpColorRepresentationManagerV1,
};
use rikka_compositor_protocols::wp_color_representation_surface_v1::{
    self, Error, WpColorRepresentationSurfaceV1,
};

use super::extension::{Claim, claim, target};
use crate::host::SurfaceHost;
use crate::ids::SurfaceId;

pub(crate) fn bound<S: SurfaceHost>(state: &mut S, manager: WpColorRepresentationManagerV1) {
    let support = &state.surfaces().color.representation;
    for mode in &support.alpha_modes {
        manager.supported_alpha_mode(*mode);
    }
    for (coefficients, range) in &support.coefficients {
        manager.supported_coefficients_and_ranges(*coefficients, *range);
    }
    manager.done();
}

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpColorRepresentationManagerV1,
    request: wp_color_representation_manager_v1::Request,
) {
    let wp_color_representation_manager_v1::Request::GetSurface { id, surface } = request else {
        return;
    };
    let claimed = claim(state, &surface, |extensions| {
        extensions.representation.is_none() && {
            extensions.representation = Some(id.clone());
            true
        }
    });
    match claimed {
        Claim::Claimed(surface) => id.set_key(surface),
        Claim::Occupied => {
            manager.post_error(wp_color_representation_manager_v1::Error::SurfaceExists)
        }
        Claim::Missing => {}
    }
}

pub(crate) fn surface<S: SurfaceHost>(
    state: &mut S,
    object: WpColorRepresentationSurfaceV1,
    request: wp_color_representation_surface_v1::Request,
) {
    use wp_color_representation_surface_v1::Request;
    if let Request::Destroy = request {
        return;
    }
    let surfaces = state.surfaces();
    let Some(surface) = object
        .key::<SurfaceId>()
        .and_then(|id| surfaces.surfaces.get_mut(id))
    else {
        object.post_error(Error::Inert);
        return;
    };
    let support = &surfaces.color.representation;
    let color = &mut surface.pending.update.color;
    match request {
        Request::Destroy => {}
        Request::SetAlphaMode { alpha_mode } => {
            match alpha_mode
                .ok()
                .filter(|mode| support.alpha_modes.contains(mode))
            {
                Some(mode) => color.alpha_mode = Some(Some(mode)),
                None => object.post_error(Error::AlphaMode),
            }
        }
        Request::SetCoefficientsAndRange {
            coefficients,
            range,
        } => {
            match coefficients
                .ok()
                .zip(range.ok())
                .filter(|pair| support.coefficients.contains(pair))
            {
                Some(pair) => color.coefficients = Some(Some(pair)),
                None => object.post_error(Error::Coefficients),
            }
        }
        Request::SetChromaLocation { chroma_location } => match chroma_location {
            Ok(location) => color.chroma_location = Some(Some(location)),
            Err(_) => object.post_error(Error::ChromaLocation),
        },
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, object: WpColorRepresentationSurfaceV1) {
    if let Some(surface) = target(state, &object)
        && surface.extensions.representation.as_ref() == Some(&object)
    {
        surface.extensions.representation = None;
        surface.pending.update.color.unset_representation();
    }
}
