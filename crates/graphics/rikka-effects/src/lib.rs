pub mod adjust;
mod curve;
mod effect;
mod error;
pub mod filter;
mod gradient;
mod hex;
mod kind;
mod knob;
mod parameters;

pub use adjust::{
    BlackWhite, BrightnessContrast, ChannelMixer, ColorBalance, ColorLookup, Curves, Exposure,
    GradientMap, HueSaturation, Levels, PhotoFilter, Posterize, SelectiveColor, ShadowsHighlights,
    Threshold, Vibrance, WhiteBalance,
};
pub use curve::{Curve, spline_of};
pub use effect::{Effect, Kind};
pub use error::{Error, Result};
pub use filter::{
    AddNoise, BoxBlur, ChannelShift, ChromaKey, Clarity, Clipping, Dither, DropShadow, Emboss,
    GaussianBlur, Glow, HighPass, LensBlur, LumaKey, Median, Mosaic, MotionBlur, OilPaint, Outline,
    RadialBlur, ReduceNoise, SmartSharpen, SurfaceBlur, TiltShift, UnsharpMask, Vignette,
};
pub use gradient::Gradient;
pub use kind::Family;
pub use knob::{Knob, Scale, Unit};
pub use parameters::{KnobKey, Knobs, Parameters};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
