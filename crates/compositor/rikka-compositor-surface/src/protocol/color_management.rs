use rikka_compositor_protocols::wp_color_management_output_v1::{self, WpColorManagementOutputV1};
use rikka_compositor_protocols::wp_color_management_surface_feedback_v1::{
    self, WpColorManagementSurfaceFeedbackV1,
};
use rikka_compositor_protocols::wp_color_management_surface_v1::{
    self, WpColorManagementSurfaceV1,
};
use rikka_compositor_protocols::wp_color_manager_v1::{
    self, Feature, Primaries, RenderIntent, TransferFunction, WpColorManagerV1,
};
use rikka_compositor_protocols::wp_image_description_creator_params_v1::{
    self, WpImageDescriptionCreatorParamsV1,
};
use rikka_compositor_protocols::wp_image_description_v1::{Cause, WpImageDescriptionV1};

use super::extension::{Claim, claim, target};
use super::image_description::{described, failed, ready};
use rikka_colorimetry::{Luminance, Mastering};

use rikka_compositor_types::OutputId;

use crate::color::{Creator, Finished, Parametric};
use crate::host::SurfaceHost;
use crate::ids::{CreatorId, SurfaceId};

const SECOND_REVISION: u32 = 2;
const THIRD_REVISION: u32 = 3;
const SCRGB_LUMINANCE: Luminance = Luminance {
    min: 0.0,
    max: 80.0,
    reference: 80.0,
};
const SCRGB_TARGET_PEAK: f64 = 10_000.0;

pub(crate) fn bound<S: SurfaceHost>(state: &mut S, manager: WpColorManagerV1) {
    let version = manager.version();
    let support = &state.surfaces().color.support;
    for intent in &support.intents {
        if *intent != RenderIntent::AbsoluteNoAdaptation || version >= SECOND_REVISION {
            manager.supported_intent(*intent);
        }
    }
    for feature in &support.features {
        let advertised = match feature {
            Feature::IccV2V4 => false,
            Feature::WindowsBt2100 => version >= THIRD_REVISION,
            _ => true,
        };
        if advertised {
            manager.supported_feature(*feature);
        }
    }
    for transfer in &support.transfer_functions {
        let advertised = match transfer {
            TransferFunction::Srgb | TransferFunction::ExtSrgb => version < SECOND_REVISION,
            TransferFunction::CompoundPower24 => version >= SECOND_REVISION,
            _ => true,
        };
        if advertised {
            manager.supported_tf_named(*transfer);
        }
    }
    for primaries in &support.primaries {
        manager.supported_primaries_named(*primaries);
    }
    manager.done();
}

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpColorManagerV1,
    request: wp_color_manager_v1::Request,
) {
    use wp_color_manager_v1::Request;
    match request {
        Request::Destroy => {}
        Request::GetOutput { id, output } => {
            if let Some(key) = state.output_of(&output) {
                id.set_key(key);
                state.surfaces().color.output_objects.push((id, key));
            }
        }
        Request::GetSurface { id, surface } => {
            match claim(state, &surface, |extensions| {
                !std::mem::replace(&mut extensions.color, true)
            }) {
                Claim::Claimed(surface) => id.set_key(surface),
                Claim::Occupied => manager.post_error(wp_color_manager_v1::Error::SurfaceExists),
                Claim::Missing => {}
            }
        }
        Request::GetSurfaceFeedback { id, surface } => {
            let surfaces = state.surfaces();
            if let Some(surface) = surfaces.lookup(&surface) {
                id.set_key(surface);
                if let Some(target) = surfaces.surfaces.get_mut(surface) {
                    target.extensions.color_feedback.push(id);
                }
            }
        }
        Request::CreateIccCreator { .. } => {
            manager.post_error(wp_color_manager_v1::Error::UnsupportedFeature);
        }
        Request::CreateParametricCreator { obj } => {
            let surfaces = state.surfaces();
            if surfaces.color.support.has(Feature::Parametric) {
                obj.set_key(surfaces.objects.creators.insert(Creator::default()));
            } else {
                manager.post_error(wp_color_manager_v1::Error::UnsupportedFeature);
            }
        }
        Request::CreateWindowsScrgb { image_description } => {
            let mut parametric = Parametric::named(Primaries::Srgb, TransferFunction::ExtLinear)
                .expect("extended linear is a known transfer function");
            parametric.description.luminance = SCRGB_LUMINANCE;
            parametric.description.mastering = Some(Mastering {
                primaries: parametric.description.primaries,
                min: SCRGB_LUMINANCE.min,
                max: SCRGB_TARGET_PEAK,
            });
            predefined(
                state,
                &manager,
                &image_description,
                Feature::WindowsScrgb,
                parametric,
            );
        }
        Request::CreateWindowsBt2100 { image_description } => {
            let parametric = Parametric::named(Primaries::Bt2020, TransferFunction::St2084Pq)
                .expect("PQ is a known transfer function");
            predefined(
                state,
                &manager,
                &image_description,
                Feature::WindowsBt2100,
                parametric,
            );
        }
        Request::GetImageDescription {
            image_description, ..
        } => failed(
            &image_description,
            Cause::Unsupported,
            "unknown image description reference",
        ),
    }
}

fn predefined<S: SurfaceHost>(
    state: &mut S,
    manager: &WpColorManagerV1,
    object: &WpImageDescriptionV1,
    feature: Feature,
    parametric: Parametric,
) {
    let surfaces = state.surfaces();
    if !surfaces.color.support.has(feature) {
        manager.post_error(wp_color_manager_v1::Error::UnsupportedFeature);
        return;
    }
    let description = surfaces.describe(parametric);
    ready(surfaces, object, description, false);
}

pub(crate) fn output<S: SurfaceHost>(
    state: &mut S,
    output: WpColorManagementOutputV1,
    request: wp_color_management_output_v1::Request,
) {
    let wp_color_management_output_v1::Request::GetImageDescription { image_description } = request
    else {
        return;
    };
    let surfaces = state.surfaces();
    let description = output
        .key::<OutputId>()
        .filter(|key| {
            surfaces
                .color
                .output_objects
                .iter()
                .any(|(object, known)| object == &output && known == key)
        })
        .and_then(|key| surfaces.color.outputs.get(&key).cloned());
    match description {
        Some(description) => ready(surfaces, &image_description, description, true),
        None => failed(
            &image_description,
            Cause::NoOutput,
            "output no longer exists",
        ),
    }
}

pub(crate) fn surface<S: SurfaceHost>(
    state: &mut S,
    object: WpColorManagementSurfaceV1,
    request: wp_color_management_surface_v1::Request,
) {
    use wp_color_management_surface_v1::{Error, Request};
    if let Request::Destroy = request {
        return;
    }
    let surfaces = state.surfaces();
    let requested = match &request {
        Request::SetImageDescription {
            image_description, ..
        } => described(surfaces, image_description),
        _ => None,
    };
    let Some(surface) = object
        .key::<SurfaceId>()
        .and_then(|id| surfaces.surfaces.get_mut(id))
    else {
        object.post_error(Error::Inert);
        return;
    };
    let intents = &surfaces.color.support.intents;
    let color = &mut surface.pending.update.color;
    match request {
        Request::Destroy => {}
        Request::UnsetImageDescription => color.description = Some(None),
        Request::SetImageDescription { render_intent, .. } => {
            let Some(description) = requested else {
                object.post_error(Error::ImageDescription);
                return;
            };
            let Some(intent) = render_intent.ok().filter(|intent| intents.contains(intent)) else {
                object.post_error(Error::RenderIntent);
                return;
            };
            color.description = Some(Some((description, intent)));
        }
    }
}

pub(crate) fn surface_destroyed<S: SurfaceHost>(state: &mut S, object: WpColorManagementSurfaceV1) {
    if let Some(surface) = target(state, &object) {
        surface.extensions.color = false;
        surface.pending.update.color.description = Some(None);
    }
}

pub(crate) fn feedback<S: SurfaceHost>(
    state: &mut S,
    object: WpColorManagementSurfaceFeedbackV1,
    request: wp_color_management_surface_feedback_v1::Request,
) {
    use wp_color_management_surface_feedback_v1::{Error, Request};
    let image_description = match request {
        Request::Destroy => return,
        Request::GetPreferred { image_description } => image_description,
        Request::GetPreferredParametric { image_description } => {
            if !state.surfaces().color.support.has(Feature::Parametric) {
                object.post_error(Error::UnsupportedFeature);
                return;
            }
            image_description
        }
    };
    let surfaces = state.surfaces();
    let Some(surface) = object
        .key::<SurfaceId>()
        .filter(|surface| surfaces.surfaces.contains_key(*surface))
    else {
        object.post_error(Error::Inert);
        return;
    };
    match surfaces.preferred_description(surface) {
        Some(description) => ready(surfaces, &image_description, description, true),
        None => failed(
            &image_description,
            Cause::Unsupported,
            "no preferred image description",
        ),
    }
}

pub(crate) fn creator<S: SurfaceHost>(
    state: &mut S,
    object: WpImageDescriptionCreatorParamsV1,
    request: wp_image_description_creator_params_v1::Request,
) {
    use wp_image_description_creator_params_v1::Request;
    let Some(id) = object.key::<CreatorId>() else {
        return;
    };
    let version = object.version();
    let surfaces = state.surfaces();
    let outcome = match request {
        Request::Create { image_description } => {
            let taken = surfaces.objects.creators.remove(id).unwrap_or_default();
            taken.finish(version).map(|finished| match finished {
                Finished::Described(parametric) => {
                    let description = surfaces.describe(*parametric);
                    ready(surfaces, &image_description, description, false);
                }
                Finished::Unsupported(reason) => {
                    failed(&image_description, Cause::Unsupported, reason);
                }
            })
        }
        request => {
            let support = &surfaces.color.support;
            let Some(creator) = surfaces.objects.creators.get_mut(id) else {
                return;
            };
            match request {
                Request::Create { .. } => Ok(()),
                Request::SetTfNamed { tf } => creator.transfer_named(support, tf.ok()),
                Request::SetTfPower { eexp } => creator.transfer_power(support, eexp),
                Request::SetPrimariesNamed { primaries } => {
                    creator.primaries_named(support, primaries.ok())
                }
                Request::SetPrimaries {
                    r_x,
                    r_y,
                    g_x,
                    g_y,
                    b_x,
                    b_y,
                    w_x,
                    w_y,
                } => creator.primaries(support, [r_x, r_y, g_x, g_y, b_x, b_y, w_x, w_y]),
                Request::SetLuminances {
                    min_lum,
                    max_lum,
                    reference_lum,
                } => creator.luminances(support, min_lum, max_lum, reference_lum),
                Request::SetMasteringDisplayPrimaries {
                    r_x,
                    r_y,
                    g_x,
                    g_y,
                    b_x,
                    b_y,
                    w_x,
                    w_y,
                } => creator.mastering_primaries(support, [r_x, r_y, g_x, g_y, b_x, b_y, w_x, w_y]),
                Request::SetMasteringLuminance { min_lum, max_lum } => {
                    creator.mastering_luminance(support, min_lum, max_lum)
                }
                Request::SetMaxCll { max_cll } => creator.max_cll(max_cll),
                Request::SetMaxFall { max_fall } => creator.max_fall(max_fall),
            }
        }
    };
    if let Err(error) = outcome {
        object.post_error(error);
    }
}

pub(crate) fn creator_destroyed<S: SurfaceHost>(
    state: &mut S,
    object: WpImageDescriptionCreatorParamsV1,
) {
    if let Some(id) = object.key::<CreatorId>() {
        state.surfaces().objects.creators.remove(id);
    }
}
