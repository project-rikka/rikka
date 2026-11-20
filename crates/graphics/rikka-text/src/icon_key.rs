use crate::icon_font::IconFont;
use crate::icon_spec::IconSpec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconKey {
    pub(crate) glyph: char,
    pub(crate) font: IconFont,
    pub(crate) size_bits: u32,
    pub(crate) optical_bits: u32,
    pub(crate) fill_bits: u32,
    pub(crate) weight_bits: u32,
    pub(crate) grade_bits: u32,
}

impl IconKey {
    pub(crate) fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }

    pub(crate) fn optical_size(&self) -> f32 {
        f32::from_bits(self.optical_bits)
    }
}

impl From<&IconSpec> for IconKey {
    fn from(spec: &IconSpec) -> Self {
        Self {
            glyph: spec.glyph,
            font: spec.font,
            size_bits: spec.size.max(1.0).to_bits(),
            optical_bits: spec.optical_size.max(1.0).to_bits(),
            fill_bits: spec.fill.to_bits(),
            weight_bits: spec.weight.0.to_bits(),
            grade_bits: spec.grade.to_bits(),
        }
    }
}
