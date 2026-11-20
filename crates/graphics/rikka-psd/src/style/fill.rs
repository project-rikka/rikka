use crate::color::Color;
use crate::descriptor::{Descriptor, Unit, Value};
use crate::style::fields::{
    angle, color, color_value, flag, number, percent, percent_value, point, point_value,
};
use crate::style::gradient::{Gradient, GradientStyle, Interpolation};

#[derive(Debug, Clone, PartialEq)]
pub struct GradientFill {
    pub gradient: Gradient,
    pub style: GradientStyle,
    pub angle: f64,
    pub reverse: bool,
    pub dither: bool,
    pub align: bool,
    pub scale: f64,
    pub offset: (f64, f64),
    pub interpolation: Interpolation,
}

impl GradientFill {
    pub fn new(gradient: Gradient) -> Self {
        Self {
            gradient,
            style: GradientStyle::Linear,
            angle: 90.0,
            reverse: false,
            dither: false,
            align: true,
            scale: 1.0,
            offset: (0.0, 0.0),
            interpolation: Interpolation::Classic,
        }
    }

    pub(crate) fn from_descriptor(descriptor: &Descriptor) -> Option<Self> {
        let gradient = Gradient::from_descriptor(descriptor.descriptor("Grad")?);
        let (x, y) = point(descriptor, "Ofst");
        Some(Self {
            gradient,
            style: GradientStyle::from_descriptor(descriptor),
            angle: number(descriptor, "Angl", 0.0),
            reverse: flag(descriptor, "Rvrs", false),
            dither: flag(descriptor, "Dthr", false),
            align: flag(descriptor, "Algn", true),
            scale: percent(descriptor, "Scl ", 1.0),
            offset: (x / 100.0, y / 100.0),
            interpolation: Interpolation::from_descriptor(descriptor),
        })
    }

    pub(crate) fn write(&self, descriptor: &mut Descriptor) {
        descriptor.set("Grad", Value::Descriptor(self.gradient.to_descriptor()));
        descriptor.set("Angl", angle(self.angle));
        descriptor.set("Type", self.style.value());
        descriptor.set("Rvrs", self.reverse);
        descriptor.set("Dthr", self.dither);
        descriptor.set("Algn", self.align);
        descriptor.set("Scl ", percent_value(self.scale));
        descriptor.set(
            "Ofst",
            point_value(self.offset.0 * 100.0, self.offset.1 * 100.0, Unit::PERCENT),
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatternFill {
    pub name: String,
    pub id: String,
    pub scale: f64,
    pub angle: f64,
    pub align: bool,
    pub phase: (f64, f64),
}

impl PatternFill {
    pub(crate) fn from_descriptor(descriptor: &Descriptor) -> Option<Self> {
        let pattern = descriptor.descriptor("Ptrn")?;
        Some(Self {
            name: pattern.string("Nm  ").unwrap_or_default().to_owned(),
            id: pattern.string("Idnt").unwrap_or_default().to_owned(),
            scale: percent(descriptor, "Scl ", 1.0),
            angle: number(descriptor, "Angl", 0.0),
            align: flag(descriptor, "Algn", true),
            phase: point(descriptor, "phase"),
        })
    }

    pub(crate) fn write(&self, descriptor: &mut Descriptor) {
        descriptor.set(
            "Ptrn",
            Value::Descriptor(
                Descriptor::new("Ptrn")
                    .with("Nm  ", self.name.as_str())
                    .with("Idnt", self.id.as_str()),
            ),
        );
        descriptor.set("Scl ", percent_value(self.scale));
        descriptor.set("Angl", angle(self.angle));
        descriptor.set("Algn", self.align);
        descriptor.set(
            "phase",
            Value::Descriptor(
                Descriptor::new("Pnt ")
                    .with("Hrzn", self.phase.0)
                    .with("Vrtc", self.phase.1),
            ),
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FillContent {
    Solid(Color),
    Gradient(GradientFill),
    Pattern(PatternFill),
}

impl FillContent {
    pub fn from_descriptor(descriptor: &Descriptor) -> Option<Self> {
        if let Some(fill) = GradientFill::from_descriptor(descriptor) {
            return Some(FillContent::Gradient(fill));
        }
        if let Some(fill) = PatternFill::from_descriptor(descriptor) {
            return Some(FillContent::Pattern(fill));
        }
        color(descriptor, "Clr ").map(FillContent::Solid)
    }

    pub fn class(&self) -> &'static str {
        match self {
            FillContent::Solid(_) => "solidColorLayer",
            FillContent::Gradient(_) => "gradientLayer",
            FillContent::Pattern(_) => "patternLayer",
        }
    }

    pub fn info_key(&self) -> &'static [u8; 4] {
        match self {
            FillContent::Solid(_) => b"SoCo",
            FillContent::Gradient(_) => b"GdFl",
            FillContent::Pattern(_) => b"PtFl",
        }
    }

    pub fn write(&self, descriptor: &mut Descriptor) {
        match self {
            FillContent::Solid(fill) => descriptor.set("Clr ", color_value(fill)),
            FillContent::Gradient(fill) => fill.write(descriptor),
            FillContent::Pattern(fill) => fill.write(descriptor),
        }
    }

    pub fn to_descriptor(&self) -> Descriptor {
        let mut descriptor = Descriptor::new(self.class());
        self.write(&mut descriptor);
        descriptor
    }
}
