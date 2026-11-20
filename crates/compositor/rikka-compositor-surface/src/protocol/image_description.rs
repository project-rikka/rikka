use rikka_compositor_protocols::wp_image_description_info_v1::WpImageDescriptionInfoV1;
use rikka_compositor_protocols::wp_image_description_v1::{self, Cause, WpImageDescriptionV1};

use rikka_colorimetry::Mastering;

use crate::color::{
    ImageDescription, Transfer, scaled_chromaticities, scaled_exponent, scaled_min_luminance,
};
use crate::host::SurfaceHost;
use crate::ids::DescriptionId;
use crate::surfaces::Surfaces;

const IDENTITY64_SINCE: u32 = 2;

#[derive(Debug)]
pub(crate) struct DescriptionObject {
    description: ImageDescription,
    informative: bool,
}

pub(crate) fn ready(
    surfaces: &mut Surfaces,
    object: &WpImageDescriptionV1,
    description: ImageDescription,
    informative: bool,
) {
    let identity = description.identity();
    let id = surfaces.objects.descriptions.insert(DescriptionObject {
        description,
        informative,
    });
    object.set_key(id);
    if object.version() >= IDENTITY64_SINCE {
        object.ready2((identity >> 32) as u32, identity as u32);
    } else {
        object.ready(identity as u32);
    }
}

pub(crate) fn failed(object: &WpImageDescriptionV1, cause: Cause, message: &str) {
    object.failed(cause, message);
}

pub(crate) fn described(
    surfaces: &Surfaces,
    object: &WpImageDescriptionV1,
) -> Option<ImageDescription> {
    let id = object.key::<DescriptionId>()?;
    Some(surfaces.objects.descriptions.get(id)?.description.clone())
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    object: WpImageDescriptionV1,
    request: wp_image_description_v1::Request,
) {
    let wp_image_description_v1::Request::GetInformation { information } = request else {
        return;
    };
    let surfaces = state.surfaces();
    let described = object
        .key::<DescriptionId>()
        .and_then(|id| surfaces.objects.descriptions.get(id));
    match described {
        Some(DescriptionObject {
            description,
            informative: true,
        }) => inform(&information, description),
        Some(_) => object.post_error(wp_image_description_v1::Error::NoInformation),
        None => object.post_error(wp_image_description_v1::Error::NotReady),
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, object: WpImageDescriptionV1) {
    if let Some(id) = object.key::<DescriptionId>() {
        state.surfaces().objects.descriptions.remove(id);
    }
}

fn inform(info: &WpImageDescriptionInfoV1, described: &ImageDescription) {
    let parametric = described.parametric();
    let description = &parametric.description;
    let [rx, ry, gx, gy, bx, by, wx, wy] = scaled_chromaticities(&description.primaries);
    info.primaries(rx, ry, gx, gy, bx, by, wx, wy);
    if let Some(named) = parametric.named_primaries {
        info.primaries_named(named);
    }
    match parametric.transfer {
        Transfer::Named(transfer) => info.tf_named(transfer),
        Transfer::Power(exponent) => info.tf_power(scaled_exponent(exponent)),
    }
    let luminance = description.luminance;
    info.luminances(
        scaled_min_luminance(luminance.min),
        luminance.max.round() as u32,
        luminance.reference.round() as u32,
    );
    let mastering = description.mastering.unwrap_or(Mastering {
        primaries: description.primaries,
        min: luminance.min,
        max: luminance.max,
    });
    let [rx, ry, gx, gy, bx, by, wx, wy] = scaled_chromaticities(&mastering.primaries);
    info.target_primaries(rx, ry, gx, gy, bx, by, wx, wy);
    info.target_luminance(
        scaled_min_luminance(mastering.min),
        mastering.max.round() as u32,
    );
    if let Some(cll) = description.max_cll {
        info.target_max_cll(cll.round() as u32);
    }
    if let Some(fall) = description.max_fall {
        info.target_max_fall(fall.round() as u32);
    }
    info.done();
}
