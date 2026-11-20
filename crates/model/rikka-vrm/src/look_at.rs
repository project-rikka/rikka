use rikka_model_core::glam::Vec3;

use rikka_model_core::Conversion;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LookAtKind {
    #[default]
    Bone,
    Expression,
}

impl LookAtKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bone => "bone",
            Self::Expression => "expression",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bone" | "Bone" => Some(Self::Bone),
            "expression" | "BlendShape" => Some(Self::Expression),
            _ => None,
        }
    }

    #[must_use]
    pub fn default_output_scale(self) -> f32 {
        match self {
            Self::Bone => 10.0,
            Self::Expression => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeMap {
    pub input_max_degrees: f32,
    pub output_scale: f32,
}

impl RangeMap {
    #[must_use]
    pub fn new(input_max_degrees: f32, output_scale: f32) -> Self {
        Self {
            input_max_degrees,
            output_scale,
        }
    }

    #[must_use]
    pub fn apply(&self, degrees: f32) -> f32 {
        let limit = self.input_max_degrees.max(0.001);
        degrees.abs().min(limit) / limit * self.output_scale
    }
}

impl Default for RangeMap {
    fn default() -> Self {
        Self::new(90.0, 10.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookAt {
    pub offset_from_head: Vec3,
    pub kind: LookAtKind,
    pub horizontal_inner: RangeMap,
    pub horizontal_outer: RangeMap,
    pub vertical_down: RangeMap,
    pub vertical_up: RangeMap,
}

impl Default for LookAt {
    fn default() -> Self {
        Self::for_kind(LookAtKind::Bone)
    }
}

impl LookAt {
    #[must_use]
    pub fn for_kind(kind: LookAtKind) -> Self {
        let range = RangeMap::new(90.0, kind.default_output_scale());
        Self {
            offset_from_head: Vec3::new(0.0, 0.06, 0.0),
            kind,
            horizontal_inner: range,
            horizontal_outer: range,
            vertical_down: range,
            vertical_up: range,
        }
    }

    pub(crate) fn convert(&mut self, conversion: &Conversion) {
        self.offset_from_head = conversion.point(self.offset_from_head);
    }
}
