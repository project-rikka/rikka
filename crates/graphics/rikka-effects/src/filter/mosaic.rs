use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Mosaic;
    CELL = "cell" => cell: || tr!("effects.adjust.cell_size"), Scale::new(2.0..=2000.0)
        .slide(2.0..=200.0)
        .unit(Unit::Pixels);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mosaic {
    cell: f32,
}

impl Default for Mosaic {
    fn default() -> Mosaic {
        Mosaic { cell: 10.0 }
    }
}

impl Parameters for Mosaic {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::Mosaic::new(self.cell).into())
    }

    fn padding(&self) -> f32 {
        self.cell
    }
}
