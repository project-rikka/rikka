mod auto;
mod blur;
mod color;
mod convert;
mod distort;
mod fractal;
mod geometry;
mod gradient;
mod group;
mod key;
mod matrix;
mod matte;
mod noise;
pub(crate) mod passes;
mod place;
pub(crate) mod resizing;
mod sharpen;
mod spline;
mod stamp;
mod stylize;
mod tone;

use crate::error::Result;
use crate::marks::Marks;
use crate::picture::Frame;
use crate::work::Work;

pub use auto::{AutoColor, AutoContrast, AutoLevels, Equalize};
pub use blur::{
    BoxBlur, GaussianBlur, LensBlur, Median, MotionBlur, Quality, RadialBlur, SurfaceBlur,
    TiltShift,
};
pub use color::{
    BlackWhite, ChannelMixer, ColorBalance, ColorLookup, Colors, Desaturate, GradientMap,
    GradientStop, HueSaturation, Hues, PhotoFilter, Primary, SelectiveColor, Sepia, Shift,
    Vibrance, WhiteBalance,
};
pub use convert::ConvertProfile;
pub use distort::{Displace, LensDistortion};
pub use fractal::FractalNoise;
pub use geometry::{
    CanvasSize, Crop, Fill, Flip, QuarterTurn, Resampling, Resize, Rotate, Transform,
};
pub use gradient::{Gradient, GradientInk, GradientShape};
pub use group::Group;
pub use key::{ChromaKey, Clipping, LumaKey, Lumas};
pub use matte::{ChannelShift, DropShadow, Outline};
pub use noise::{AddNoise, Distribution, ReduceNoise};
pub use place::Place;
pub use sharpen::{Clarity, HighPass, Removal, SmartSharpen, UnsharpMask};
pub use spline::Spline;
pub use stamp::Stamp;
pub use stylize::{
    Dither, Dithering, Emboss, FindEdges, Glow, Mosaic, OilPaint, ShadowsHighlights, Vignette,
};
pub use tone::{
    BrightnessContrast, Channel, Curves, Exposure, Invert, Level, Levels, Posterize, Threshold,
};

pub(crate) mod kind {
    pub(crate) const CURVES: u32 = 0;
    pub(crate) const EXPOSURE: u32 = 1;
    pub(crate) const HUE_SATURATION: u32 = 2;
    pub(crate) const COLOR_BALANCE: u32 = 3;
    pub(crate) const BLACK_WHITE: u32 = 4;
    pub(crate) const PHOTO_FILTER: u32 = 5;
    pub(crate) const CHANNEL_MIXER: u32 = 6;
    pub(crate) const INVERT: u32 = 7;
    pub(crate) const THRESHOLD: u32 = 8;
    pub(crate) const GRADIENT_MAP: u32 = 9;
    pub(crate) const SELECTIVE_COLOR: u32 = 10;
    pub(crate) const VIBRANCE: u32 = 11;
    pub(crate) const LOOKUP: u32 = 12;
    pub(crate) const DESATURATE: u32 = 13;
    pub(crate) const WHITE_BALANCE: u32 = 14;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reach {
    Global,
    Everywhere { halo: u32 },
    Within { area: [u32; 4], halo: u32 },
    Nowhere,
}

impl Reach {
    pub(crate) const POINTWISE: Reach = Reach::Everywhere { halo: 0 };

    pub(crate) fn bounded(area: [u32; 4]) -> Reach {
        Reach::Within { area, halo: 0 }
    }

    fn halo(self) -> u32 {
        match self {
            Reach::Everywhere { halo } | Reach::Within { halo, .. } => halo,
            Reach::Global | Reach::Nowhere => 0,
        }
    }

    pub(crate) fn intersect(self, other: Reach) -> Reach {
        let halo = self.halo() + other.halo();
        match (self, other) {
            (Reach::Global, _) | (_, Reach::Global) => Reach::Global,
            (Reach::Nowhere, _) | (_, Reach::Nowhere) => Reach::Nowhere,
            (Reach::Everywhere { .. }, Reach::Everywhere { .. }) => Reach::Everywhere { halo },
            (Reach::Everywhere { .. }, Reach::Within { area, .. })
            | (Reach::Within { area, .. }, Reach::Everywhere { .. }) => {
                Reach::Within { area, halo }
            }
            (Reach::Within { area: a, .. }, Reach::Within { area: b, .. }) => {
                let left = a[0].max(b[0]);
                let top = a[1].max(b[1]);
                let right = (a[0] + a[2]).min(b[0] + b[2]);
                let bottom = (a[1] + a[3]).min(b[1] + b[3]);
                match right > left && bottom > top {
                    true => Reach::Within {
                        area: [left, top, right - left, bottom - top],
                        halo,
                    },
                    false => Reach::Nowhere,
                }
            }
        }
    }

    pub(crate) fn union(self, other: Reach) -> Reach {
        let halo = self.halo() + other.halo();
        match (self, other) {
            (Reach::Global, _) | (_, Reach::Global) => Reach::Global,
            (Reach::Everywhere { .. }, _) | (_, Reach::Everywhere { .. }) => {
                Reach::Everywhere { halo }
            }
            (Reach::Nowhere, reach) | (reach, Reach::Nowhere) => reach,
            (Reach::Within { area: a, .. }, Reach::Within { area: b, .. }) => {
                let left = a[0].min(b[0]);
                let top = a[1].min(b[1]);
                let right = (a[0] + a[2]).max(b[0] + b[2]);
                let bottom = (a[1] + a[3]).max(b[1] + b[3]);
                Reach::Within {
                    area: [left, top, right - left, bottom - top],
                    halo,
                }
            }
        }
    }
}

pub(crate) trait Effect: Clone {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame>;

    fn reach(&self, _extent: [u32; 2]) -> Reach {
        Reach::Global
    }

    fn scaled(&self, _factor: f32) -> Self {
        self.clone()
    }

    fn reshapes(&self) -> bool {
        false
    }
}

macro_rules! operations {
    ($($name:ident),* $(,)?) => {
        #[derive(Clone, Debug, PartialEq)]
        pub enum Operation {
            $($name($name),)*
        }

        $(
            impl From<$name> for Operation {
                fn from(operation: $name) -> Operation {
                    Operation::$name(operation)
                }
            }

            impl From<$name> for crate::recipe::Step {
                fn from(operation: $name) -> crate::recipe::Step {
                    crate::recipe::Step::new(operation)
                }
            }
        )*

        impl Operation {
            pub(crate) fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
                match self {
                    $(Operation::$name(operation) => operation.apply(work, frame),)*
                }
            }

            #[must_use]
            pub(crate) fn scaled(&self, factor: f32) -> Operation {
                match self {
                    $(Operation::$name(operation) => Operation::$name(operation.scaled(factor)),)*
                }
            }

            pub(crate) fn reach(&self, extent: [u32; 2]) -> Reach {
                match self {
                    $(Operation::$name(operation) => operation.reach(extent),)*
                }
            }

            #[must_use]
            pub fn reshapes(&self) -> bool {
                match self {
                    $(Operation::$name(operation) => operation.reshapes(),)*
                }
            }
        }
    };
}

operations! {
    Levels,
    Curves,
    BrightnessContrast,
    Exposure,
    Posterize,
    Invert,
    Threshold,
    HueSaturation,
    ColorBalance,
    BlackWhite,
    Sepia,
    PhotoFilter,
    ChannelMixer,
    GradientMap,
    SelectiveColor,
    Vibrance,
    ColorLookup,
    Desaturate,
    WhiteBalance,
    AutoLevels,
    AutoContrast,
    AutoColor,
    Equalize,
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
    FindEdges,
    Mosaic,
    OilPaint,
    Vignette,
    Glow,
    Dither,
    ShadowsHighlights,
    Resize,
    Rotate,
    QuarterTurn,
    Flip,
    Crop,
    Fill,
    CanvasSize,
    Transform,
    ConvertProfile,
    Marks,
    Stamp,
    Gradient,
    Group,
    Place,
    ChromaKey,
    LumaKey,
    ChannelShift,
    Clipping,
    DropShadow,
    Outline,
    FractalNoise,
    Displace,
    LensDistortion,
}
