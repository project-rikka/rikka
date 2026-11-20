pub mod add_noise;
pub mod box_blur;
pub mod channel_shift;
pub mod chroma_key;
pub mod clarity;
pub mod clipping;
pub mod dither;
pub mod drop_shadow;
pub mod emboss;
pub mod gaussian_blur;
pub mod glow;
pub mod high_pass;
pub mod lens_blur;
pub mod luma_key;
pub mod median;
pub mod mosaic;
pub mod motion_blur;
pub mod oil_paint;
pub mod outline;
pub mod radial_blur;
pub mod reduce_noise;
pub mod smart_sharpen;
pub mod surface_blur;
pub mod tilt_shift;
pub mod unsharp_mask;
pub mod vignette;

use crate::knob::{Scale, Unit};

pub use add_noise::AddNoise;
pub use box_blur::BoxBlur;
pub use channel_shift::ChannelShift;
pub use chroma_key::ChromaKey;
pub use clarity::Clarity;
pub use clipping::Clipping;
pub use dither::Dither;
pub use drop_shadow::DropShadow;
pub use emboss::Emboss;
pub use gaussian_blur::GaussianBlur;
pub use glow::Glow;
pub use high_pass::HighPass;
pub use lens_blur::LensBlur;
pub use luma_key::LumaKey;
pub use median::Median;
pub use mosaic::Mosaic;
pub use motion_blur::MotionBlur;
pub use oil_paint::OilPaint;
pub use outline::Outline;
pub use radial_blur::RadialBlur;
pub use reduce_noise::ReduceNoise;
pub use smart_sharpen::SmartSharpen;
pub use surface_blur::SurfaceBlur;
pub use tilt_shift::TiltShift;
pub use unsharp_mask::UnsharpMask;
pub use vignette::Vignette;

pub const RADIUS: Scale = Scale::new(0.1..=1000.0)
    .slide(0.1..=250.0)
    .decimals(1)
    .unit(Unit::Pixels);
pub const PERCENT: Scale = Scale::new(0.0..=100.0).unit(Unit::Percent);
pub const TURN: Scale = Scale::new(-180.0..=180.0).unit(Unit::Degrees);
pub const HALF_TURN: Scale = Scale::new(-90.0..=90.0).unit(Unit::Degrees);
