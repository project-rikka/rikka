use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::{Label, tr};
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for LensBlur;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(0.0..=500.0)
            .slide(0.0..=100.0)
            .unit(Unit::Pixels);
    CURVATURE = "curvature" => curvature: || tr!("effects.lens_blur.curvature"), Scale::new(0.0..=100.0);
    ROTATION = "rotation" => rotation: || tr!("effects.lens_blur.rotation"), Scale::new(0.0..=360.0).unit(Unit::Degrees);
    BRIGHTNESS = "brightness" => brightness: || tr!("effects.adjust.brightness"), Scale::new(0.0..=100.0);
    THRESHOLD = "threshold" => threshold: || tr!("effects.adjust.threshold"), Scale::new(0.0..=255.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Iris {
    Circle,
    Triangle,
    Square,
    Pentagon,
    #[default]
    Hexagon,
    Heptagon,
    Octagon,
}

impl Iris {
    pub fn blades(self) -> u32 {
        match self {
            Iris::Circle => 0,
            Iris::Triangle => 3,
            Iris::Square => 4,
            Iris::Pentagon => 5,
            Iris::Hexagon => 6,
            Iris::Heptagon => 7,
            Iris::Octagon => 8,
        }
    }

    pub fn name(self) -> Label {
        match self {
            Iris::Circle => tr!("effects.lens_blur.circle"),
            Iris::Triangle => tr!("effects.lens_blur.triangle"),
            Iris::Square => tr!("effects.lens_blur.square"),
            Iris::Pentagon => tr!("effects.lens_blur.pentagon"),
            Iris::Hexagon => tr!("effects.lens_blur.hexagon"),
            Iris::Heptagon => tr!("effects.lens_blur.heptagon"),
            Iris::Octagon => tr!("effects.lens_blur.octagon"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LensBlur {
    radius: f32,
    pub shape: Iris,
    curvature: f32,
    rotation: f32,
    brightness: f32,
    threshold: f32,
}

impl Default for LensBlur {
    fn default() -> LensBlur {
        LensBlur {
            radius: 15.0,
            shape: Iris::default(),
            curvature: 0.0,
            rotation: 0.0,
            brightness: 0.0,
            threshold: 255.0,
        }
    }
}

impl Parameters for LensBlur {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::LensBlur::new(self.radius)
                .iris(self.shape.blades(), self.rotation, self.curvature / 100.0)
                .specular(self.brightness, self.threshold / 255.0)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.radius
    }
}
