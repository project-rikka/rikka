use super::PERCENT;
use crate::hex;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use rikka_types::Color;
use serde::{Deserialize, Serialize};

knobs! {
    for ChromaKey;
    TOLERANCE = "tolerance" => tolerance: || tr!("effects.key.tolerance"), PERCENT;
    SOFTNESS = "softness" => softness: || tr!("effects.key.softness"), PERCENT;
    SPILL = "spill" => spill: || tr!("effects.key.spill"), PERCENT;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromaKey {
    #[serde(with = "hex")]
    pub color: Color,
    tolerance: f32,
    softness: f32,
    spill: f32,
}

impl Default for ChromaKey {
    fn default() -> ChromaKey {
        ChromaKey {
            color: Color::hex(0x00FF00),
            tolerance: 25.0,
            softness: 10.0,
            spill: 50.0,
        }
    }
}

impl Parameters for ChromaKey {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::ChromaKey::new(self.color)
                .tolerance(self.tolerance)
                .softness(self.softness)
                .spill(self.spill)
                .into(),
        )
    }
}
