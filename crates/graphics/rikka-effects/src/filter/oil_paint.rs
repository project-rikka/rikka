use super::TURN;
use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const TENTHS: Scale = Scale::new(0.0..=10.0).decimals(1);

knobs! {
    for OilPaint;
    STYLIZATION = "stylization" => stylization: || tr!("effects.oil_paint.stylization"), Scale::new(0.1..=10.0).decimals(1);
    CLEANLINESS = "cleanliness" => cleanliness: || tr!("effects.oil_paint.cleanliness"), TENTHS;
    SCALE = "scale" => scale: || tr!("effects.oil_paint.scale"), Scale::new(0.1..=10.0).decimals(1);
    BRISTLE_DETAIL = "bristle_detail" => bristle_detail: || tr!("effects.oil_paint.bristle_detail"), TENTHS;
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), TURN;
    SHINE = "shine" => shine: || tr!("effects.oil_paint.shine"), TENTHS;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OilPaint {
    stylization: f32,
    cleanliness: f32,
    scale: f32,
    bristle_detail: f32,
    angle: f32,
    shine: f32,
}

impl Default for OilPaint {
    fn default() -> OilPaint {
        OilPaint {
            stylization: 2.5,
            cleanliness: 5.0,
            scale: 1.0,
            bristle_detail: 3.0,
            angle: -60.0,
            shine: 1.0,
        }
    }
}

impl Parameters for OilPaint {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::OilPaint::new()
                .brush(
                    self.stylization,
                    self.cleanliness,
                    self.scale,
                    self.bristle_detail,
                )
                .lighting(self.angle, self.shine)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        (self.stylization * self.scale * 1.5).max(1.0)
    }
}
