use rikka_colorimetry::{BlendSpace, Description, Intent, Mapping, Primaries, TransferFunction};
use rikka_compositor_surface::{
    AlphaMode, ChromaLocation, Coefficients, ColorRange, ColorRepresentation, RenderIntent, Surface,
};
use rikka_gpu::{ChromaSiting, Ycbcr, YcbcrMatrix, YcbcrRange};
use rikka_renderer::{AlphaMode as Alpha, ImageColor};
use rikka_types::Color;

const REFERENCE_MATCH: f64 = 0.5;
const PRIMARIES_MATCH: f64 = 1.0e-4;

pub(crate) fn image_color(surface: &Surface) -> ImageColor {
    let described = surface.image_description();
    ImageColor {
        description: described.map_or(Description::SRGB, |(description, _)| {
            *description.description()
        }),
        intent: match described.map(|(_, intent)| intent) {
            Some(RenderIntent::Relative) => Intent::Relative,
            _ => Intent::Perceptual,
        },
        alpha: match surface.color_representation().alpha_mode {
            Some(AlphaMode::PremultipliedOptical) => Alpha::PremultipliedOptical,
            Some(AlphaMode::Straight) => Alpha::Straight,
            Some(AlphaMode::PremultipliedElectrical) | None => Alpha::Premultiplied,
        },
    }
}

pub(crate) fn ycbcr(representation: &ColorRepresentation, height: u32) -> Ycbcr {
    let guessed = Ycbcr::guessed(height);
    let (matrix, range) = match representation.coefficients {
        Some((coefficients, range)) => (
            match coefficients {
                Coefficients::Bt601 => YcbcrMatrix::Bt601,
                Coefficients::Bt2020 => YcbcrMatrix::Bt2020,
                _ => YcbcrMatrix::Bt709,
            },
            match range {
                ColorRange::Full => YcbcrRange::Full,
                ColorRange::Limited => YcbcrRange::Limited,
            },
        ),
        None => (guessed.matrix, guessed.range),
    };
    let siting = match representation.chroma_location {
        Some(ChromaLocation::Type0) => ChromaSiting::Left,
        Some(ChromaLocation::Type1) | None => ChromaSiting::Centre,
        Some(ChromaLocation::Type2) => ChromaSiting::TopLeft,
        Some(ChromaLocation::Type3) => ChromaSiting::Top,
        Some(ChromaLocation::Type4) => ChromaSiting::BottomLeft,
        Some(ChromaLocation::Type5) => ChromaSiting::Bottom,
    };
    Ycbcr {
        matrix,
        range,
        siting,
    }
}

pub(crate) fn scans_out_unchanged(color: &ImageColor, space: &BlendSpace) -> bool {
    if color.alpha != Alpha::Premultiplied {
        return false;
    }
    let description = &color.description;
    let Some(mapping) = Mapping::new(description, color.intent, space) else {
        return false;
    };
    match space {
        BlendSpace::Sdr => mapping.is_identity(),
        BlendSpace::Hdr(hdr) => {
            description.transfer == TransferFunction::Pq
                && mapping.tone.is_none()
                && same_primaries(&description.primaries, &Primaries::BT2020)
                && (description.luminance.reference - hdr.reference).abs() < REFERENCE_MATCH
        }
    }
}

pub(crate) fn solid_in_srgb(color: Color, image: &ImageColor) -> Color {
    if *image == ImageColor::default() || color.a <= 0.0 {
        return color;
    }
    let Some(mapping) = Mapping::new(&image.description, image.intent, &BlendSpace::Sdr) else {
        return color;
    };
    let [r, g, b] = mapping
        .apply([color.r, color.g, color.b].map(f64::from))
        .map(|channel| channel as f32);
    Color::rgba(r, g, b, color.a)
}

fn same_primaries(one: &Primaries, other: &Primaries) -> bool {
    [
        (one.red, other.red),
        (one.green, other.green),
        (one.blue, other.blue),
        (one.white, other.white),
    ]
    .iter()
    .all(|(a, b)| (a.x - b.x).abs() < PRIMARIES_MATCH && (a.y - b.y).abs() < PRIMARIES_MATCH)
}
