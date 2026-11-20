use rikka_i18n::{Label, tr};

use crate::effect::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Adjust,
    Blur,
    Sharpen,
    Noise,
    Stylize,
    Photo,
    Key,
}

impl Family {
    pub const ALL: [Family; 7] = [
        Family::Adjust,
        Family::Blur,
        Family::Sharpen,
        Family::Noise,
        Family::Stylize,
        Family::Photo,
        Family::Key,
    ];

    pub fn name(self) -> Label {
        match self {
            Family::Adjust => tr!("effects.family.adjust"),
            Family::Blur => tr!("effects.family.blur"),
            Family::Sharpen => tr!("effects.family.sharpen"),
            Family::Noise => tr!("effects.family.noise"),
            Family::Stylize => tr!("effects.family.stylize"),
            Family::Photo => tr!("effects.family.photo"),
            Family::Key => tr!("effects.family.key"),
        }
    }

    pub fn kinds(self) -> impl Iterator<Item = Kind> {
        Kind::ALL
            .iter()
            .copied()
            .filter(move |kind| kind.family() == self)
    }
}

impl Kind {
    pub fn name(self) -> Label {
        match self {
            Kind::BrightnessContrast => tr!("effects.kind.brightness_contrast"),
            Kind::Levels => tr!("effects.kind.levels"),
            Kind::Curves => tr!("effects.kind.curves"),
            Kind::Exposure => tr!("effects.kind.exposure"),
            Kind::Vibrance => tr!("effects.kind.vibrance"),
            Kind::HueSaturation => tr!("effects.kind.hue_saturation"),
            Kind::ColorBalance => tr!("effects.kind.color_balance"),
            Kind::BlackWhite => tr!("effects.kind.black_white"),
            Kind::PhotoFilter => tr!("effects.kind.photo_filter"),
            Kind::ChannelMixer => tr!("effects.kind.channel_mixer"),
            Kind::ColorLookup => tr!("effects.kind.color_lookup"),
            Kind::WhiteBalance => tr!("effects.kind.white_balance"),
            Kind::Posterize => tr!("effects.kind.posterize"),
            Kind::Threshold => tr!("effects.kind.threshold"),
            Kind::GradientMap => tr!("effects.kind.gradient_map"),
            Kind::SelectiveColor => tr!("effects.kind.selective_color"),
            Kind::ShadowsHighlights => tr!("effects.kind.shadows_highlights"),
            Kind::GaussianBlur => tr!("effects.kind.gaussian_blur"),
            Kind::BoxBlur => tr!("effects.kind.box_blur"),
            Kind::MotionBlur => tr!("effects.kind.motion_blur"),
            Kind::RadialBlur => tr!("effects.kind.radial_blur"),
            Kind::LensBlur => tr!("effects.kind.lens_blur"),
            Kind::SurfaceBlur => tr!("effects.kind.surface_blur"),
            Kind::Median => tr!("effects.kind.median"),
            Kind::TiltShift => tr!("effects.kind.tilt_shift"),
            Kind::UnsharpMask => tr!("effects.kind.unsharp_mask"),
            Kind::SmartSharpen => tr!("effects.kind.smart_sharpen"),
            Kind::HighPass => tr!("effects.kind.high_pass"),
            Kind::Clarity => tr!("effects.kind.clarity"),
            Kind::AddNoise => tr!("effects.kind.add_noise"),
            Kind::ReduceNoise => tr!("effects.kind.reduce_noise"),
            Kind::Emboss => tr!("effects.kind.emboss"),
            Kind::Mosaic => tr!("effects.kind.mosaic"),
            Kind::OilPaint => tr!("effects.kind.oil_paint"),
            Kind::Glow => tr!("effects.kind.glow"),
            Kind::Vignette => tr!("effects.kind.vignette"),
            Kind::Dither => tr!("effects.kind.dither"),
            Kind::ChromaKey => tr!("effects.kind.chroma_key"),
            Kind::LumaKey => tr!("effects.kind.luma_key"),
            Kind::Clipping => tr!("effects.kind.clipping"),
            Kind::ChannelShift => tr!("effects.kind.channel_shift"),
            Kind::DropShadow => tr!("effects.kind.drop_shadow"),
            Kind::Outline => tr!("effects.kind.outline"),
        }
    }

    pub fn family(self) -> Family {
        match self {
            Kind::BrightnessContrast
            | Kind::Levels
            | Kind::Curves
            | Kind::Exposure
            | Kind::Vibrance
            | Kind::HueSaturation
            | Kind::ColorBalance
            | Kind::BlackWhite
            | Kind::PhotoFilter
            | Kind::ChannelMixer
            | Kind::ColorLookup
            | Kind::WhiteBalance
            | Kind::Posterize
            | Kind::Threshold
            | Kind::GradientMap
            | Kind::SelectiveColor
            | Kind::ShadowsHighlights => Family::Adjust,
            Kind::GaussianBlur
            | Kind::BoxBlur
            | Kind::MotionBlur
            | Kind::RadialBlur
            | Kind::LensBlur
            | Kind::SurfaceBlur
            | Kind::Median
            | Kind::TiltShift => Family::Blur,
            Kind::UnsharpMask | Kind::SmartSharpen | Kind::HighPass | Kind::Clarity => {
                Family::Sharpen
            }
            Kind::AddNoise | Kind::ReduceNoise => Family::Noise,
            Kind::Emboss
            | Kind::Mosaic
            | Kind::OilPaint
            | Kind::Glow
            | Kind::ChannelShift
            | Kind::DropShadow
            | Kind::Outline => Family::Stylize,
            Kind::Vignette | Kind::Dither => Family::Photo,
            Kind::ChromaKey | Kind::LumaKey | Kind::Clipping => Family::Key,
        }
    }

    pub fn wants_histogram(self) -> bool {
        matches!(self, Kind::Levels | Kind::Curves | Kind::Threshold)
    }
}
