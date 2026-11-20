use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for SurfaceBlur;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(1.0..=100.0).unit(Unit::Pixels);
    THRESHOLD = "threshold" => threshold: || tr!("effects.adjust.threshold"), Scale::new(2.0..=255.0);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceBlur {
    radius: f32,
    threshold: f32,
}

impl Default for SurfaceBlur {
    fn default() -> SurfaceBlur {
        SurfaceBlur {
            radius: 5.0,
            threshold: 15.0,
        }
    }
}

impl Parameters for SurfaceBlur {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::SurfaceBlur::new(self.radius, self.threshold).into())
    }

    fn padding(&self) -> f32 {
        self.radius
    }
}
