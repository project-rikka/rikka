use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation, Quality};
use serde::{Deserialize, Serialize};

const CENTER: Scale = Scale::new(0.0..=1.0).decimals(2);

knobs! {
    for RadialBlur;
    SPIN = "spin" => spin: || tr!("effects.adjust.angle"), Scale::new(1.0..=360.0)
            .slide(1.0..=90.0)
            .unit(Unit::Degrees);
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(1.0..=100.0);
    CENTER_X = "center_x" => center_x: || tr!("effects.radial_blur.center_x"), CENTER;
    CENTER_Y = "center_y" => center_y: || tr!("effects.radial_blur.center_y"), CENTER;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    #[default]
    Spin,
    Zoom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Draft,
    #[default]
    Good,
    Best,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RadialBlur {
    pub method: Method,
    pub spin: f32,
    amount: f32,
    pub quality: Grade,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for RadialBlur {
    fn default() -> RadialBlur {
        RadialBlur {
            method: Method::default(),
            spin: 10.0,
            amount: 10.0,
            quality: Grade::default(),
            center_x: 0.5,
            center_y: 0.5,
        }
    }
}

impl Parameters for RadialBlur {
    fn operation(&self) -> Option<Operation> {
        let blur = match self.method {
            Method::Zoom => imaging::RadialBlur::zoom(self.amount),
            Method::Spin => imaging::RadialBlur::spin(self.spin),
        };
        let quality = match self.quality {
            Grade::Draft => Quality::Draft,
            Grade::Good => Quality::Good,
            Grade::Best => Quality::Best,
        };
        Some(
            blur.center(self.center_x, self.center_y)
                .quality(quality)
                .into(),
        )
    }
}
