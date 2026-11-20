use crate::color::Color;
use crate::descriptor::Descriptor;
use crate::info::Adjustment;
use crate::style::fields::{color, flag, number};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vibrance {
    pub vibrance: f64,
    pub saturation: f64,
}

impl Vibrance {
    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        Self {
            vibrance: number(descriptor, "vibrance", 0.0),
            saturation: number(descriptor, "Strt", 0.0),
        }
    }

    pub fn to_descriptor(&self) -> Descriptor {
        Descriptor::new("null")
            .with("vibrance", self.vibrance.round() as i32)
            .with("Strt", self.saturation.round() as i32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlackWhite {
    pub reds: f64,
    pub yellows: f64,
    pub greens: f64,
    pub cyans: f64,
    pub blues: f64,
    pub magentas: f64,
    pub tint: Option<Color>,
}

impl BlackWhite {
    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        Self {
            reds: number(descriptor, "Rd  ", 40.0),
            yellows: number(descriptor, "Yllw", 60.0),
            greens: number(descriptor, "Grn ", 40.0),
            cyans: number(descriptor, "Cyn ", 60.0),
            blues: number(descriptor, "Bl  ", 20.0),
            magentas: number(descriptor, "Mgnt", 80.0),
            tint: flag(descriptor, "useTint", false)
                .then(|| color(descriptor, "tintColor"))
                .flatten(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutFormat {
    Cube,
    ThreeDl,
    Look,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorLookup {
    pub name: String,
    pub format: LutFormat,
    pub data: Vec<u8>,
    pub profile: Vec<u8>,
    pub dither: bool,
}

impl ColorLookup {
    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        let format = match descriptor.enumeration("LUTFormat").as_deref() {
            Some("LUTFormatCUBE") => LutFormat::Cube,
            Some("LUTFormat3DL") => LutFormat::ThreeDl,
            Some("LUTFormatLOOK") => LutFormat::Look,
            _ => LutFormat::Other,
        };
        Self {
            name: descriptor
                .string("LUT3DFileName")
                .or_else(|| descriptor.string("Nm  "))
                .unwrap_or_default()
                .to_owned(),
            format,
            data: descriptor
                .bytes("LUT3DFileData")
                .unwrap_or_default()
                .to_vec(),
            profile: descriptor.bytes("profile").unwrap_or_default().to_vec(),
            dither: flag(descriptor, "Dthr", false),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdjustmentLayer {
    Binary(Adjustment),
    Vibrance(Vibrance),
    BlackWhite(BlackWhite),
    ColorLookup(ColorLookup),
}
