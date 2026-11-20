use rikka_effects::{Effect, Parameters};
use rikka_ui::Element;

use crate::tuner::Tuner;
use crate::tuning::Tuning;

pub(crate) trait Form: Parameters {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element;
}

pub fn form<S: 'static>(effect: &Effect, tuning: &Tuning<S>) -> Element {
    match effect {
        Effect::BrightnessContrast(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Levels(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Curves(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Exposure(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Vibrance(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::HueSaturation(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ColorBalance(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::BlackWhite(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::PhotoFilter(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ChannelMixer(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ColorLookup(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::WhiteBalance(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Posterize(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Threshold(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::GradientMap(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::SelectiveColor(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ShadowsHighlights(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::GaussianBlur(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::BoxBlur(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::MotionBlur(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::RadialBlur(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::LensBlur(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::SurfaceBlur(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Median(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::TiltShift(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::UnsharpMask(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::SmartSharpen(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::HighPass(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Clarity(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::AddNoise(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ReduceNoise(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Emboss(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Mosaic(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::OilPaint(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Glow(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Vignette(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Dither(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ChromaKey(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::LumaKey(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Clipping(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::ChannelShift(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::DropShadow(parameters) => Tuner::new(tuning, parameters).form(),
        Effect::Outline(parameters) => Tuner::new(tuning, parameters).form(),
    }
}
