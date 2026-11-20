use rikka_text_font::Tag;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub tag: Tag,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorFormats {
    pub colr: bool,
    pub cbdt: bool,
    pub sbix: bool,
    pub svg: bool,
}

impl ColorFormats {
    pub fn any(self) -> bool {
        self.colr || self.cbdt || self.sbix || self.svg
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisRange {
    pub(crate) min: f32,
    pub(crate) default: f32,
    pub(crate) max: f32,
}

impl AxisRange {
    pub(crate) fn fixed(value: f32) -> Self {
        Self {
            min: value,
            default: value,
            max: value,
        }
    }

    #[inline]
    pub(crate) fn contains(&self, value: f32) -> bool {
        self.min <= value && value <= self.max
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slope {
    pub(crate) upright: bool,
    pub(crate) italic: bool,
    pub(crate) oblique: Option<(f32, f32)>,
}

impl Slope {
    pub(crate) const UPRIGHT: Self = Self {
        upright: true,
        italic: false,
        oblique: None,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FaceClass {
    Sans,
    Serif,
    Mono,
    Other,
}

impl From<&Axis> for AxisRange {
    fn from(axis: &Axis) -> Self {
        Self {
            min: axis.min,
            default: axis.default,
            max: axis.max,
        }
    }
}
