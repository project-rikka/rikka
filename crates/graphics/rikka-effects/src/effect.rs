use rikka_image_edit::Recipe;
use serde::{Deserialize, Serialize};

use crate::adjust::{
    BlackWhite, BrightnessContrast, ChannelMixer, ColorBalance, ColorLookup, Curves, Exposure,
    GradientMap, HueSaturation, Levels, PhotoFilter, Posterize, SelectiveColor, ShadowsHighlights,
    Threshold, Vibrance, WhiteBalance,
};
use crate::filter::{
    AddNoise, BoxBlur, ChannelShift, ChromaKey, Clarity, Clipping, Dither, DropShadow, Emboss,
    GaussianBlur, Glow, HighPass, LensBlur, LumaKey, Median, Mosaic, MotionBlur, OilPaint, Outline,
    RadialBlur, ReduceNoise, SmartSharpen, SurfaceBlur, TiltShift, UnsharpMask, Vignette,
};
use crate::knob::Knob;
use crate::parameters::{self, Knobs, Parameters};

macro_rules! catalog {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum Effect {
            $($variant($variant),)+
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Kind {
            $($variant,)+
        }

        impl Kind {
            pub const ALL: &'static [Kind] = &[$(Kind::$variant,)+];

            pub fn default_effect(self) -> Effect {
                match self {
                    $(Kind::$variant => Effect::$variant($variant::default()),)+
                }
            }
        }

        impl Effect {
            pub fn kind(&self) -> Kind {
                match self {
                    $(Effect::$variant(_) => Kind::$variant,)+
                }
            }
        }

        $(
            impl From<$variant> for Effect {
                fn from(parameters: $variant) -> Effect {
                    Effect::$variant(parameters)
                }
            }
        )+

        impl Effect {
            pub fn recipe(&self) -> Recipe {
                match self {
                    $(Effect::$variant(parameters) => parameters::recipe(parameters),)+
                }
            }

            pub fn padding(&self) -> f32 {
                match self {
                    $(Effect::$variant(parameters) => parameters.padding(),)+
                }
            }

            pub fn knobs(&self) -> &'static [Knob] {
                match self {
                    $(Effect::$variant(parameters) => parameters.knobs(),)+
                }
            }

            pub fn value(&self, key: &str) -> Option<f32> {
                match self {
                    $(Effect::$variant(parameters) => parameters.peek(key),)+
                }
            }

            pub fn set(&mut self, key: &str, value: f32) -> bool {
                match self {
                    $(Effect::$variant(parameters) => parameters.set(key, value),)+
                }
            }

            #[must_use]
            pub fn checked(self) -> Effect {
                match self {
                    $(Effect::$variant(parameters) => parameters::checked(parameters).into(),)+
                }
            }
        }
    };
}

catalog! {
    BrightnessContrast,
    Levels,
    Curves,
    Exposure,
    Vibrance,
    HueSaturation,
    ColorBalance,
    BlackWhite,
    PhotoFilter,
    ChannelMixer,
    ColorLookup,
    WhiteBalance,
    Posterize,
    Threshold,
    GradientMap,
    SelectiveColor,
    ShadowsHighlights,
    GaussianBlur,
    BoxBlur,
    MotionBlur,
    RadialBlur,
    LensBlur,
    SurfaceBlur,
    Median,
    TiltShift,
    UnsharpMask,
    SmartSharpen,
    HighPass,
    Clarity,
    AddNoise,
    ReduceNoise,
    Emboss,
    Mosaic,
    OilPaint,
    Glow,
    Vignette,
    Dither,
    ChromaKey,
    LumaKey,
    Clipping,
    ChannelShift,
    DropShadow,
    Outline,
}
