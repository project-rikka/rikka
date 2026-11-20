use rikka_app::prelude::{Label, MaterialSymbol, tr};
use rikka_effects::Kind;
use rikka_image_edit::{Flip, Operation, QuarterTurn};

pub(crate) fn name(operation: &Operation) -> Label {
    match operation {
        Operation::Levels(_) => Kind::Levels.name(),
        Operation::Curves(_) => Kind::Curves.name(),
        Operation::BrightnessContrast(_) => Kind::BrightnessContrast.name(),
        Operation::Exposure(_) => Kind::Exposure.name(),
        Operation::Posterize(_) => Kind::Posterize.name(),
        Operation::Invert(_) => tr!("paint.operation.invert"),
        Operation::Threshold(_) => Kind::Threshold.name(),
        Operation::HueSaturation(_) => Kind::HueSaturation.name(),
        Operation::ColorBalance(_) => Kind::ColorBalance.name(),
        Operation::BlackWhite(_) => Kind::BlackWhite.name(),
        Operation::Sepia(_) => tr!("paint.operation.sepia"),
        Operation::PhotoFilter(_) => Kind::PhotoFilter.name(),
        Operation::ChannelMixer(_) => Kind::ChannelMixer.name(),
        Operation::GradientMap(_) => Kind::GradientMap.name(),
        Operation::SelectiveColor(_) => Kind::SelectiveColor.name(),
        Operation::Vibrance(_) => Kind::Vibrance.name(),
        Operation::ColorLookup(_) => Kind::ColorLookup.name(),
        Operation::Desaturate(_) => tr!("paint.operation.desaturate"),
        Operation::WhiteBalance(_) => Kind::WhiteBalance.name(),
        Operation::AutoLevels(_) => tr!("paint.operation.auto_levels"),
        Operation::AutoContrast(_) => tr!("paint.operation.auto_contrast"),
        Operation::AutoColor(_) => tr!("paint.operation.auto_color"),
        Operation::Equalize(_) => tr!("paint.operation.equalize"),
        Operation::GaussianBlur(_) => Kind::GaussianBlur.name(),
        Operation::BoxBlur(_) => Kind::BoxBlur.name(),
        Operation::MotionBlur(_) => Kind::MotionBlur.name(),
        Operation::RadialBlur(_) => Kind::RadialBlur.name(),
        Operation::LensBlur(_) => Kind::LensBlur.name(),
        Operation::SurfaceBlur(_) => Kind::SurfaceBlur.name(),
        Operation::Median(_) => Kind::Median.name(),
        Operation::TiltShift(_) => Kind::TiltShift.name(),
        Operation::UnsharpMask(_) => Kind::UnsharpMask.name(),
        Operation::SmartSharpen(_) => Kind::SmartSharpen.name(),
        Operation::HighPass(_) => Kind::HighPass.name(),
        Operation::Clarity(_) => Kind::Clarity.name(),
        Operation::AddNoise(_) => Kind::AddNoise.name(),
        Operation::ReduceNoise(_) => Kind::ReduceNoise.name(),
        Operation::Emboss(_) => Kind::Emboss.name(),
        Operation::FindEdges(_) => tr!("paint.operation.find_edges"),
        Operation::Mosaic(_) => Kind::Mosaic.name(),
        Operation::OilPaint(_) => Kind::OilPaint.name(),
        Operation::Vignette(_) => Kind::Vignette.name(),
        Operation::Glow(_) => Kind::Glow.name(),
        Operation::Dither(_) => Kind::Dither.name(),
        Operation::ShadowsHighlights(_) => Kind::ShadowsHighlights.name(),
        Operation::Resize(_) => tr!("paint.operation.resize"),
        Operation::Rotate(_) => tr!("paint.operation.rotate"),
        Operation::QuarterTurn(turn) => turn_name(*turn),
        Operation::Flip(flip) => flip_name(*flip),
        Operation::Crop(_) => tr!("paint.operation.crop"),
        Operation::Fill(_) => tr!("paint.operation.fill"),
        Operation::CanvasSize(_) => tr!("paint.operation.canvas_size"),
        Operation::Transform(_) => tr!("paint.operation.transform"),
        Operation::ConvertProfile(_) => tr!("paint.operation.convert_profile"),
        Operation::Marks(marks) => crate::tool::drawn(marks)
            .map_or_else(|| tr!("paint.operation.marks"), crate::tool::Tool::name),
        Operation::Stamp(_) => tr!("paint.action.text"),
        Operation::Gradient(_) => tr!("paint.operation.gradient"),
        Operation::Group(_) => tr!("paint.operation.group"),
        Operation::Place(_) => tr!("paint.operation.place"),
        Operation::ChromaKey(_) => Kind::ChromaKey.name(),
        Operation::LumaKey(_) => Kind::LumaKey.name(),
        Operation::ChannelShift(_) => Kind::ChannelShift.name(),
        Operation::Clipping(_) => Kind::Clipping.name(),
        Operation::DropShadow(_) => Kind::DropShadow.name(),
        Operation::Outline(_) => Kind::Outline.name(),
        Operation::FractalNoise(_) => tr!("paint.operation.fractal_noise"),
        Operation::Displace(_) => tr!("paint.operation.displace"),
        Operation::LensDistortion(_) => tr!("paint.operation.lens_distortion"),
    }
}

pub(crate) fn turn_name(turn: QuarterTurn) -> Label {
    match turn {
        QuarterTurn::Clockwise => tr!("paint.operation.turn_clockwise"),
        QuarterTurn::CounterClockwise => tr!("paint.operation.turn_counter_clockwise"),
        QuarterTurn::Half => tr!("paint.operation.turn_half"),
    }
}

pub(crate) fn flip_name(flip: Flip) -> Label {
    match flip {
        Flip::Horizontal => tr!("paint.operation.flip_horizontal"),
        Flip::Vertical => tr!("paint.operation.flip_vertical"),
    }
}

pub(crate) fn symbol(operation: &Operation) -> MaterialSymbol {
    match operation {
        Operation::Levels(_)
        | Operation::Curves(_)
        | Operation::BrightnessContrast(_)
        | Operation::Exposure(_)
        | Operation::Posterize(_)
        | Operation::Threshold(_)
        | Operation::ShadowsHighlights(_)
        | Operation::AutoLevels(_)
        | Operation::AutoContrast(_)
        | Operation::Equalize(_) => MaterialSymbol::TUNE,
        Operation::Invert(_) => MaterialSymbol::INVERT_COLORS,
        Operation::HueSaturation(_)
        | Operation::ColorBalance(_)
        | Operation::BlackWhite(_)
        | Operation::Sepia(_)
        | Operation::PhotoFilter(_)
        | Operation::ChannelMixer(_)
        | Operation::GradientMap(_)
        | Operation::SelectiveColor(_)
        | Operation::Vibrance(_)
        | Operation::ColorLookup(_)
        | Operation::Desaturate(_)
        | Operation::WhiteBalance(_)
        | Operation::AutoColor(_) => MaterialSymbol::PALETTE,
        Operation::GaussianBlur(_)
        | Operation::BoxBlur(_)
        | Operation::MotionBlur(_)
        | Operation::RadialBlur(_)
        | Operation::LensBlur(_)
        | Operation::SurfaceBlur(_)
        | Operation::Median(_)
        | Operation::TiltShift(_) => MaterialSymbol::BLUR_ON,
        Operation::UnsharpMask(_)
        | Operation::SmartSharpen(_)
        | Operation::HighPass(_)
        | Operation::Clarity(_) => MaterialSymbol::DEBLUR,
        Operation::AddNoise(_)
        | Operation::ReduceNoise(_)
        | Operation::Dither(_)
        | Operation::FractalNoise(_) => MaterialSymbol::GRAIN,
        Operation::Emboss(_)
        | Operation::FindEdges(_)
        | Operation::Mosaic(_)
        | Operation::OilPaint(_)
        | Operation::Vignette(_)
        | Operation::Glow(_) => MaterialSymbol::AUTO_AWESOME,
        Operation::Resize(_) => MaterialSymbol::PHOTO_SIZE_SELECT_LARGE,
        Operation::Rotate(_) | Operation::QuarterTurn(_) => MaterialSymbol::ROTATE_RIGHT,
        Operation::Flip(_) => MaterialSymbol::FLIP,
        Operation::Crop(_) => MaterialSymbol::CROP,
        Operation::Fill(_) => MaterialSymbol::FORMAT_COLOR_FILL,
        Operation::CanvasSize(_) => MaterialSymbol::ASPECT_RATIO,
        Operation::Transform(_) => MaterialSymbol::TRANSFORM,
        Operation::ConvertProfile(_) => MaterialSymbol::COLOR_LENS,
        Operation::Marks(marks) => {
            crate::tool::drawn(marks).map_or(MaterialSymbol::BRUSH, crate::tool::Tool::symbol)
        }
        Operation::Stamp(_) => MaterialSymbol::TEXT_FIELDS,
        Operation::Gradient(_) => MaterialSymbol::GRADIENT,
        Operation::Group(_) | Operation::Place(_) => MaterialSymbol::LAYERS,
        Operation::ChromaKey(_) | Operation::LumaKey(_) => MaterialSymbol::FILTER_B_AND_W,
        Operation::Clipping(_) => MaterialSymbol::CROP_FREE,
        Operation::ChannelShift(_)
        | Operation::DropShadow(_)
        | Operation::Outline(_)
        | Operation::Displace(_)
        | Operation::LensDistortion(_) => MaterialSymbol::AUTO_AWESOME,
    }
}
