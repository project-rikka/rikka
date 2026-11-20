use crate::curve_profile::CurveProfile;
use crate::error::{Error, Result};

pub const MAX_SEGMENTS: u32 = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affect {
    Vertices,
    #[default]
    Edges,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WidthType {
    #[default]
    Offset,
    Width,
    Depth,
    Percent,
    Absolute,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ProfileType {
    #[default]
    Superellipse,
    Custom(CurveProfile),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OuterMiter {
    #[default]
    Sharp,
    Patch,
    Arc,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InnerMiter {
    #[default]
    Sharp,
    Arc,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Intersection {
    #[default]
    GridFill,
    Cutoff,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaceStrength {
    #[default]
    None,
    New,
    Affected,
    All,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BevelOptions {
    pub affect: Affect,
    pub width_type: WidthType,
    pub width: f32,
    pub segments: u32,
    pub profile_shape: f32,
    pub profile_type: ProfileType,
    pub material: Option<u32>,
    pub harden_normals: bool,
    pub clamp_overlap: bool,
    pub loop_slide: bool,
    pub mark_seam: bool,
    pub mark_sharp: bool,
    pub miter_outer: OuterMiter,
    pub miter_inner: InnerMiter,
    pub spread: f32,
    pub intersection: Intersection,
    pub face_strength: FaceStrength,
}

impl Default for BevelOptions {
    fn default() -> BevelOptions {
        BevelOptions {
            affect: Affect::Edges,
            width_type: WidthType::Offset,
            width: 0.0,
            segments: 1,
            profile_shape: 0.5,
            profile_type: ProfileType::Superellipse,
            material: None,
            harden_normals: false,
            clamp_overlap: false,
            loop_slide: true,
            mark_seam: false,
            mark_sharp: false,
            miter_outer: OuterMiter::Sharp,
            miter_inner: InnerMiter::Sharp,
            spread: 0.1,
            intersection: Intersection::GridFill,
            face_strength: FaceStrength::None,
        }
    }
}

impl BevelOptions {
    #[must_use]
    pub fn modifier() -> BevelOptions {
        BevelOptions {
            width: 0.1,
            clamp_overlap: true,
            ..BevelOptions::default()
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.width.is_finite() || self.width < 0.0 {
            return Err(Error::InvalidWidth(self.width));
        }
        if self.width_type == WidthType::Percent && self.width > 100.0 {
            return Err(Error::InvalidPercent(self.width));
        }
        if self.segments == 0 || self.segments > MAX_SEGMENTS {
            return Err(Error::InvalidSegments(self.segments));
        }
        if !(0.0..=1.0).contains(&self.profile_shape) {
            return Err(Error::InvalidProfileShape(self.profile_shape));
        }
        if !self.spread.is_finite() || self.spread < 0.0 {
            return Err(Error::InvalidSpread(self.spread));
        }
        Ok(())
    }
}
