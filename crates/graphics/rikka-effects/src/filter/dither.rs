use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Dithering, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Dither;
    LEVELS = "levels" => levels: || tr!("effects.dither.levels"), Scale::new(2.0..=256.0).slide(2.0..=32.0);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Ordered,
    #[default]
    Diffusion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dither {
    levels: f32,
    pub method: Method,
}

impl Default for Dither {
    fn default() -> Dither {
        Dither {
            levels: 4.0,
            method: Method::default(),
        }
    }
}

impl Parameters for Dither {
    fn normalize(&mut self, _changed: Option<&str>) {
        self.levels = self.levels.round();
    }

    fn operation(&self) -> Option<Operation> {
        let method = match self.method {
            Method::Ordered => Dithering::Ordered,
            Method::Diffusion => Dithering::Diffusion,
        };
        Some(
            imaging::Dither::new(self.levels as u32)
                .method(method)
                .into(),
        )
    }
}
