use std::path::Path;

use paint_document::{Combine, Error, Layer};
use rikka_app::prelude::*;
use rikka_image_edit::{GradientShape, Intent, Profile, Resampling};
use rikka_psd::BlendMode;

use crate::options::{Filling, GradientMode, SampleFrom, SampleSize};
use crate::stroke::Measure;

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub(crate) fn error_message(error: &Error) -> Label {
    match error {
        Error::Unreadable { source, .. } => {
            tr!("paint.trouble.unreadable", reason = &source.to_string())
        }
        Error::Unsupported { .. } => tr!("paint.trouble.unsupported"),
        Error::Damaged { reason, .. } => tr!("paint.trouble.damaged", reason = reason.as_str()),
        Error::TooLarge { width, height } => {
            tr!("paint.trouble.too_large", width = *width, height = *height)
        }
        Error::NoGpu => tr!("paint.trouble.no_gpu"),
        Error::Imaging(reason) => tr!("paint.trouble.engine", reason = &reason.to_string()),
        Error::Gpu(reason) => tr!("paint.trouble.engine", reason = &reason.to_string()),
        Error::Psd(reason) => tr!("paint.trouble.engine", reason = &reason.to_string()),
        Error::Picture(reason) => tr!("paint.trouble.engine", reason = &reason.to_string()),
        Error::Unwritable { source, .. } => {
            tr!("paint.trouble.unwritable", reason = &source.to_string())
        }
        Error::UnknownFont { family } => {
            tr!("paint.trouble.unknown_font", family = family.as_str())
        }
        Error::Unlettered(reason) => tr!("paint.trouble.unlettered", reason = reason.as_str()),
        Error::BackgroundLocked => tr!("paint.trouble.background_locked"),
        Error::LastLayer => tr!("paint.trouble.last_layer"),
        Error::UnknownLayer(_) => tr!("paint.trouble.unknown_layer"),
        Error::NotPixelLayer(_) => tr!("paint.trouble.not_pixel_layer"),
        Error::HiddenLayer(_) => tr!("paint.trouble.hidden_layer"),
        Error::NothingVisible => tr!("paint.trouble.nothing_visible"),
        Error::TooFewVisible => tr!("paint.trouble.too_few_visible"),
        Error::AlreadyFlat => tr!("paint.trouble.already_flat"),
        Error::NoBackground => tr!("paint.trouble.no_background"),
        Error::UnknownBlend => tr!("paint.trouble.unknown_blend"),
        Error::PassThroughOutsideGroup => tr!("paint.trouble.pass_through"),
        Error::MoveIntoItself => tr!("paint.trouble.move_into_itself"),
        Error::NotGroup => tr!("paint.trouble.not_group"),
        Error::AtTop => tr!("paint.trouble.at_top"),
        Error::AtBottom => tr!("paint.trouble.at_bottom"),
        Error::GroupMerge => tr!("paint.trouble.group_merge"),
        Error::NothingBelow => tr!("paint.trouble.nothing_below"),
        Error::ProfileNeedsRgb => tr!("paint.trouble.profile_needs_rgb"),
        Error::ProfileOnFloat => tr!("paint.trouble.profile_on_float"),
        Error::GreyProfile => tr!("paint.trouble.grey_profile"),
        Error::NothingToAdd => tr!("paint.trouble.nothing_to_add"),
        Error::FigureExists(_) | Error::UnknownFigure(_) => tr!("paint.trouble.unknown_object"),
        Error::EmptyText => tr!("paint.trouble.empty_text"),
        Error::UnboundedFigure => tr!("paint.trouble.unbounded_object"),
        Error::LayerReshape => tr!("paint.trouble.layer_reshape"),
        Error::CanvasPreview => tr!("paint.trouble.canvas_preview"),
        Error::TravelInBatch | Error::NestedBatch => tr!("paint.trouble.batch"),
        Error::OutsideImage { .. } => tr!("paint.trouble.outside_image"),
        Error::EmptyImage => tr!("paint.trouble.empty_image"),
        Error::BadResolution => tr!("paint.trouble.bad_resolution"),
        Error::SingularMap => tr!("paint.trouble.singular_map"),
    }
}

const BACKGROUND: &str = "Background";

pub(crate) fn layer_name(layer: &Layer) -> Label {
    let name = layer.name.trim();
    match (name.is_empty(), layer.background) {
        (true, true) => tr!("paint.layers.background"),
        (false, true) if name == BACKGROUND => tr!("paint.layers.background"),
        (true, false) => tr!("paint.layers.unnamed"),
        (false, _) => Label::from(layer.name.clone()),
    }
}

pub(crate) fn blend(blend: BlendMode) -> Label {
    match blend {
        BlendMode::PassThrough => tr!("paint.blend.pass_through"),
        BlendMode::Normal => tr!("paint.blend.normal"),
        BlendMode::Dissolve => tr!("paint.blend.dissolve"),
        BlendMode::Darken => tr!("paint.blend.darken"),
        BlendMode::Multiply => tr!("paint.blend.multiply"),
        BlendMode::ColorBurn => tr!("paint.blend.color_burn"),
        BlendMode::LinearBurn => tr!("paint.blend.linear_burn"),
        BlendMode::DarkerColor => tr!("paint.blend.darker_color"),
        BlendMode::Lighten => tr!("paint.blend.lighten"),
        BlendMode::Screen => tr!("paint.blend.screen"),
        BlendMode::ColorDodge => tr!("paint.blend.color_dodge"),
        BlendMode::LinearDodge => tr!("paint.blend.linear_dodge"),
        BlendMode::LighterColor => tr!("paint.blend.lighter_color"),
        BlendMode::Overlay => tr!("paint.blend.overlay"),
        BlendMode::SoftLight => tr!("paint.blend.soft_light"),
        BlendMode::HardLight => tr!("paint.blend.hard_light"),
        BlendMode::VividLight => tr!("paint.blend.vivid_light"),
        BlendMode::LinearLight => tr!("paint.blend.linear_light"),
        BlendMode::PinLight => tr!("paint.blend.pin_light"),
        BlendMode::HardMix => tr!("paint.blend.hard_mix"),
        BlendMode::Difference => tr!("paint.blend.difference"),
        BlendMode::Exclusion => tr!("paint.blend.exclusion"),
        BlendMode::Subtract => tr!("paint.blend.subtract"),
        BlendMode::Divide => tr!("paint.blend.divide"),
        BlendMode::Hue => tr!("paint.blend.hue"),
        BlendMode::Saturation => tr!("paint.blend.saturation"),
        BlendMode::Color => tr!("paint.blend.color"),
        BlendMode::Luminosity => tr!("paint.blend.luminosity"),
        BlendMode::Unknown(_) => tr!("paint.blend.unknown"),
    }
}

pub(crate) fn resampling(resampling: Resampling) -> Label {
    match resampling {
        Resampling::Nearest => tr!("paint.resampling.nearest"),
        Resampling::Bilinear => tr!("paint.resampling.bilinear"),
        Resampling::Bicubic => tr!("paint.resampling.bicubic"),
        Resampling::BicubicSmoother => tr!("paint.resampling.bicubic_smoother"),
        Resampling::Lanczos => tr!("paint.resampling.lanczos"),
    }
}

pub(crate) fn intent(intent: Intent) -> Label {
    match intent {
        Intent::Perceptual => tr!("paint.intent.perceptual"),
        Intent::RelativeColorimetric => tr!("paint.intent.relative"),
        Intent::Saturation => tr!("paint.intent.saturation"),
        Intent::AbsoluteColorimetric => tr!("paint.intent.absolute"),
    }
}

pub(crate) fn profile(profile: Option<&Profile>) -> Label {
    match profile.and_then(Profile::name) {
        Some(name) => Label::from(name.to_owned()),
        None if profile.is_some() => tr!("paint.profile.unnamed"),
        None => tr!("paint.profile.untagged"),
    }
}

pub(crate) fn color(color: Color) -> Label {
    if color.a <= 0.0 {
        return tr!("paint.color.transparent");
    }
    let hex = color.with_alpha(1.0).to_hex().to_ascii_uppercase();
    match color.a >= 1.0 {
        true => Label::from(hex),
        false => Label::from(format!("{hex} {}", percent(color.a))),
    }
}

pub(crate) fn combine(combine: Combine) -> Label {
    match combine {
        Combine::Replace => tr!("paint.options.replace"),
        Combine::Union => tr!("paint.options.add"),
        Combine::Exclude => tr!("paint.options.subtract"),
        Combine::Intersect => tr!("paint.options.intersect"),
        Combine::Xor => tr!("paint.options.invert"),
    }
}

pub(crate) fn sample_size(size: SampleSize) -> Label {
    match size {
        SampleSize::Pixel => tr!("paint.options.single_pixel"),
        SampleSize::Three => tr!("paint.options.three_average"),
        SampleSize::Five => tr!("paint.options.five_average"),
    }
}

pub(crate) fn filling(filling: Filling) -> Label {
    match filling {
        Filling::Outline => tr!("paint.options.outline"),
        Filling::Fill => tr!("paint.options.fill"),
        Filling::FillAndOutline => tr!("paint.options.fill_and_outline"),
    }
}

pub(crate) fn gradient_shape(shape: GradientShape) -> Label {
    match shape {
        GradientShape::Linear => tr!("paint.gradient.linear"),
        GradientShape::Reflected => tr!("paint.gradient.reflected"),
        GradientShape::Radial => tr!("paint.gradient.radial"),
        GradientShape::Diamond => tr!("paint.gradient.diamond"),
        GradientShape::Conical => tr!("paint.gradient.conical"),
    }
}

pub(crate) fn gradient_mode(mode: GradientMode) -> Label {
    match mode {
        GradientMode::Color => tr!("paint.gradient.color"),
        GradientMode::Transparency => tr!("paint.gradient.transparency"),
    }
}

pub(crate) fn sample_from(from: SampleFrom) -> Label {
    match from {
        SampleFrom::Image => tr!("paint.options.image"),
        SampleFrom::Layer => tr!("paint.options.layer"),
    }
}

pub(crate) fn measure(measure: Measure) -> Label {
    match measure {
        Measure::Length { pixels, degrees } => tr!(
            "paint.status.length",
            length = &format!("{pixels:.0}"),
            angle = &format!("{degrees:.1}")
        ),
        Measure::Size { width, height } => tr!(
            "paint.status.size",
            width = width.round() as u32,
            height = height.round() as u32
        ),
    }
}

pub(crate) fn percent(scale: f32) -> String {
    let percent = scale * 100.0;
    match percent >= 10.0 || (percent - percent.round()).abs() < 0.05 {
        true => format!("{}%", percent.round()),
        false => format!("{percent:.1}%"),
    }
}
