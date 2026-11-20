use crate::color::Color;
use crate::descriptor::{Descriptor, Value};
use crate::style::fields::{color, color_value, enumeration, flag, number, percent, percent_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopColor {
    User,
    Foreground,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    pub location: f64,
    pub midpoint: f64,
    pub color: Color,
    pub kind: StopColor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpacityStop {
    pub location: f64,
    pub midpoint: f64,
    pub opacity: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorModel {
    Rgb,
    Hsb,
    Lab,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GradientKind {
    Solid {
        smoothness: f64,
        colors: Vec<ColorStop>,
        opacities: Vec<OpacityStop>,
    },
    Noise {
        roughness: f64,
        model: ColorModel,
        seed: i64,
        restrict_colors: bool,
        add_transparency: bool,
        minimum: [f64; 4],
        maximum: [f64; 4],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    pub name: String,
    pub kind: GradientKind,
    source: Descriptor,
}

impl Gradient {
    pub fn two_color(from: Color, to: Color) -> Self {
        Self {
            name: String::new(),
            kind: GradientKind::Solid {
                smoothness: 1.0,
                colors: vec![
                    ColorStop {
                        location: 0.0,
                        midpoint: 0.5,
                        color: from,
                        kind: StopColor::User,
                    },
                    ColorStop {
                        location: 1.0,
                        midpoint: 0.5,
                        color: to,
                        kind: StopColor::User,
                    },
                ],
                opacities: vec![
                    OpacityStop {
                        location: 0.0,
                        midpoint: 0.5,
                        opacity: 1.0,
                    },
                    OpacityStop {
                        location: 1.0,
                        midpoint: 0.5,
                        opacity: 1.0,
                    },
                ],
            },
            source: Descriptor::new("Grdn"),
        }
    }

    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        let noise = descriptor
            .enumeration("GrdF")
            .is_some_and(|value| value == "ClNs");
        let kind = if noise {
            let range = |key: &str| {
                let mut values = [0.0; 4];
                if let Some(items) = descriptor.list(key) {
                    for (slot, item) in values.iter_mut().zip(items) {
                        *slot = item.number().unwrap_or(0.0) / 100.0;
                    }
                }
                values
            };
            let model = match descriptor.enumeration("ClrS").as_deref() {
                Some("HSBC") => ColorModel::Hsb,
                Some("LbCl") => ColorModel::Lab,
                _ => ColorModel::Rgb,
            };
            GradientKind::Noise {
                roughness: number(descriptor, "Smth", 2048.0) / 4096.0,
                model,
                seed: descriptor.integer("RndS").unwrap_or(0),
                restrict_colors: flag(descriptor, "VctC", false),
                add_transparency: flag(descriptor, "ShTr", false),
                minimum: range("Mnm "),
                maximum: range("Mxm "),
            }
        } else {
            let colors = descriptor
                .list("Clrs")
                .unwrap_or(&[])
                .iter()
                .filter_map(Value::as_descriptor)
                .map(|stop| ColorStop {
                    location: number(stop, "Lctn", 0.0) / 4096.0,
                    midpoint: number(stop, "Mdpn", 50.0) / 100.0,
                    color: color(stop, "Clr ").unwrap_or_default(),
                    kind: match stop.enumeration("Type").as_deref() {
                        Some("FrgC") => StopColor::Foreground,
                        Some("BckC") => StopColor::Background,
                        _ => StopColor::User,
                    },
                })
                .collect();
            let opacities = descriptor
                .list("Trns")
                .unwrap_or(&[])
                .iter()
                .filter_map(Value::as_descriptor)
                .map(|stop| OpacityStop {
                    location: number(stop, "Lctn", 0.0) / 4096.0,
                    midpoint: number(stop, "Mdpn", 50.0) / 100.0,
                    opacity: percent(stop, "Opct", 1.0),
                })
                .collect();
            GradientKind::Solid {
                smoothness: number(descriptor, "Intr", 4096.0) / 4096.0,
                colors,
                opacities,
            }
        };
        Self {
            name: descriptor.string("Nm  ").unwrap_or_default().to_owned(),
            kind,
            source: descriptor.clone(),
        }
    }

    pub fn to_descriptor(&self) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.set("Nm  ", self.name.as_str());
        match &self.kind {
            GradientKind::Solid {
                smoothness,
                colors,
                opacities,
            } => {
                descriptor.set("GrdF", enumeration("GrdF", "CstS"));
                descriptor.set("Intr", (smoothness * 4096.0).round());
                let colors = colors
                    .iter()
                    .map(|stop| {
                        let kind = match stop.kind {
                            StopColor::User => "UsrS",
                            StopColor::Foreground => "FrgC",
                            StopColor::Background => "BckC",
                        };
                        Value::Descriptor(
                            Descriptor::new("Clrt")
                                .with("Clr ", color_value(&stop.color))
                                .with("Type", enumeration("Clry", kind))
                                .with("Lctn", (stop.location * 4096.0).round() as i32)
                                .with("Mdpn", (stop.midpoint * 100.0).round() as i32),
                        )
                    })
                    .collect::<Vec<_>>();
                let opacities = opacities
                    .iter()
                    .map(|stop| {
                        Value::Descriptor(
                            Descriptor::new("TrnS")
                                .with("Opct", percent_value(stop.opacity))
                                .with("Lctn", (stop.location * 4096.0).round() as i32)
                                .with("Mdpn", (stop.midpoint * 100.0).round() as i32),
                        )
                    })
                    .collect::<Vec<_>>();
                descriptor.set("Clrs", Value::List(colors));
                descriptor.set("Trns", Value::List(opacities));
            }
            GradientKind::Noise {
                roughness,
                model,
                seed,
                restrict_colors,
                add_transparency,
                minimum,
                maximum,
            } => {
                descriptor.set("GrdF", enumeration("GrdF", "ClNs"));
                descriptor.set("ShTr", *add_transparency);
                descriptor.set("VctC", *restrict_colors);
                let model = match model {
                    ColorModel::Rgb => "RGBC",
                    ColorModel::Hsb => "HSBC",
                    ColorModel::Lab => "LbCl",
                };
                descriptor.set("ClrS", enumeration("ClrS", model));
                descriptor.set("RndS", *seed as i32);
                descriptor.set("Smth", (roughness * 4096.0).round() as i32);
                let range = |values: &[f64; 4]| {
                    Value::List(
                        values
                            .iter()
                            .map(|value| Value::Integer((value * 100.0).round() as i32))
                            .collect(),
                    )
                };
                descriptor.set("Mnm ", range(minimum));
                descriptor.set("Mxm ", range(maximum));
            }
        }
        descriptor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GradientStyle {
    #[default]
    Linear,
    Radial,
    Angle,
    Reflected,
    Diamond,
}

impl GradientStyle {
    pub(crate) fn from_descriptor(descriptor: &Descriptor) -> Self {
        match descriptor.enumeration("Type").as_deref() {
            Some("Rdl ") => GradientStyle::Radial,
            Some("Angl") => GradientStyle::Angle,
            Some("Rflc") => GradientStyle::Reflected,
            Some("Dmnd") => GradientStyle::Diamond,
            _ => GradientStyle::Linear,
        }
    }

    pub(crate) fn value(self) -> Value {
        enumeration(
            "GrdT",
            match self {
                GradientStyle::Linear => "Lnr ",
                GradientStyle::Radial => "Rdl ",
                GradientStyle::Angle => "Angl",
                GradientStyle::Reflected => "Rflc",
                GradientStyle::Diamond => "Dmnd",
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Interpolation {
    #[default]
    Classic,
    Perceptual,
    Linear,
    Smooth,
}

impl Interpolation {
    pub(crate) fn from_descriptor(descriptor: &Descriptor) -> Self {
        match descriptor
            .enumeration("gradientsInterpolationMethod")
            .as_deref()
        {
            Some("Perc") => Interpolation::Perceptual,
            Some("Lnr ") => Interpolation::Linear,
            Some("Smoo") | Some("smoothInterpolation") => Interpolation::Smooth,
            _ => Interpolation::Classic,
        }
    }
}
