use glam::Vec3;

use crate::look::NITS_PER_RADIANCE_UNIT;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Metering {
    #[default]
    Auto,
    Manual,
    Physical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exposure {
    pub compensation: f32,
    pub adaptation: f32,
    pub figure_metering: f32,
    pub metering: Metering,
    pub aperture: f32,
    pub shutter: f32,
    pub iso: f32,
    pub low_percentile: f32,
    pub high_percentile: f32,
    pub darkest: f32,
    pub brightest: f32,
    pub center_weight: f32,
}

impl Default for Exposure {
    fn default() -> Self {
        Self {
            compensation: 0.0,
            adaptation: 1.0,
            figure_metering: 0.25,
            metering: Metering::Auto,
            aperture: 16.0,
            shutter: 0.01,
            iso: 100.0,
            low_percentile: 0.45,
            high_percentile: 0.96,
            darkest: -3.0,
            brightest: 4.5,
            center_weight: 0.65,
        }
    }
}

impl Exposure {
    pub const SATURATION_SPEED: f32 = 1.2;

    pub fn ev100(&self) -> f32 {
        let aperture = self.aperture.max(0.1);
        let shutter = self.shutter.max(1e-6);
        let iso = self.iso.max(1.0);
        (aperture * aperture / shutter).log2() - (iso / 100.0).log2()
    }

    pub fn physical(&self) -> f32 {
        NITS_PER_RADIANCE_UNIT * self.compensation.exp2()
            / (Self::SATURATION_SPEED * self.ev100().exp2())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionBlur {
    pub amount: f32,
}

impl Default for MotionBlur {
    fn default() -> Self {
        Self { amount: 0.5 }
    }
}

impl MotionBlur {
    pub(crate) fn is_active(&self) -> bool {
        self.amount > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthOfField {
    pub focus: f32,
    pub aperture: f32,
    pub sensor: f32,
    pub blades: u32,
    pub blade_rotation: f32,
}

impl Default for DepthOfField {
    fn default() -> Self {
        Self {
            focus: 3.0,
            aperture: 2.8,
            sensor: 24.0,
            blades: 0,
            blade_rotation: 0.0,
        }
    }
}

impl DepthOfField {
    pub fn focal_length(&self, fov_y: f32) -> f32 {
        0.5 * self.sensor / (0.5 * fov_y).tan().max(1e-4)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    pub bloom: f32,
    pub radius: f32,
    pub threshold: f32,
    pub tint: Vec3,
    pub diffusion: f32,
    pub streaks: f32,
    pub streak_tint: Vec3,
    pub flare: f32,
    pub leaks: f32,
    pub leak_tint: Vec3,
    pub blades: u32,
    pub blade_rotation: f32,
}

impl Glow {
    pub const SOURCE: &'static str = "source";

    pub(crate) fn glows(&self) -> bool {
        [self.bloom, self.diffusion, self.streaks, self.flare]
            .iter()
            .any(|&amount| amount > 0.0)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.glows() || self.leaks > 0.0
    }
}

impl Default for Glow {
    fn default() -> Self {
        Self {
            bloom: 0.6,
            radius: 0.6,
            threshold: 0.0,
            tint: Vec3::ONE,
            diffusion: 0.12,
            streaks: 0.1,
            streak_tint: Vec3::new(0.6, 0.8, 1.0),
            flare: 0.15,
            leaks: 0.0,
            leak_tint: Vec3::new(1.0, 0.55, 0.3),
            blades: 0,
            blade_rotation: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    pub contrast: f32,
    pub saturation: f32,
    pub vibrance: f32,
    pub temperature: f32,
    pub tint: f32,
    pub lift: Vec3,
    pub gamma: Vec3,
    pub gain: Vec3,
    pub shadow_tint: Vec3,
    pub highlight_tint: Vec3,
    pub black: f32,
    pub night: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Self {
            contrast: 1.08,
            saturation: 1.0,
            vibrance: 0.15,
            temperature: 0.0,
            tint: 0.0,
            lift: Vec3::ZERO,
            gamma: Vec3::ONE,
            gain: Vec3::ONE,
            shadow_tint: Vec3::ONE,
            highlight_tint: Vec3::ONE,
            black: 0.0,
            night: 1.0,
        }
    }
}

impl Grade {
    pub(crate) fn grades_display(&self) -> bool {
        self.lift != Vec3::ZERO
            || self.gamma != Vec3::ONE
            || self.gain != Vec3::ONE
            || self.shadow_tint != Vec3::ONE
            || self.highlight_tint != Vec3::ONE
            || self.black != 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    #[default]
    Gt,
    Agx,
    Aces,
    Neutral,
    Clamp,
    Hable,
    Reinhard,
    Lottes,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AgxLook {
    Base,
    #[default]
    Punchy,
    Golden,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneCurve {
    pub peak: f32,
    pub contrast: f32,
    pub toe: f32,
    pub linear: f32,
    pub toe_power: f32,
}

impl Default for ToneCurve {
    fn default() -> Self {
        Self {
            peak: 1.0,
            contrast: 1.0,
            toe: 0.22,
            linear: 0.4,
            toe_power: 1.33,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tonemap {
    pub tone: Tone,
    pub look: AgxLook,
    pub preserve_hue: f32,
    pub curve: ToneCurve,
}

impl Tonemap {
    pub fn new(tone: Tone) -> Self {
        Self {
            tone,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    pub chromatic_aberration: f32,
    pub vignette: f32,
    pub grain: f32,
    pub sharpen: f32,
}

impl Default for Lens {
    fn default() -> Self {
        Self {
            chromatic_aberration: 0.2,
            vignette: 0.3,
            grain: 0.08,
            sharpen: 0.2,
        }
    }
}

impl Lens {
    pub(crate) fn is_active(&self) -> bool {
        [
            self.chromatic_aberration,
            self.vignette,
            self.grain,
            self.sharpen,
        ]
        .iter()
        .any(|&amount| amount > 0.0)
    }
}
