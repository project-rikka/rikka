use std::fmt;
use std::sync::Arc;

use crate::buffer::glyph_buffer::Buffer;
use crate::common::direction::Direction;
use crate::common::feature::Feature;
use crate::common::segment_properties::SegmentProperties;
use crate::face::shape_face::ShapeFace;
use crate::plan::feature_map::FeatureMap;
use crate::shapers::complex_shaper::{ComplexShaper, ShaperData};

pub(crate) struct PlanCore {
    pub(crate) properties: SegmentProperties,
    pub(crate) features: Box<[Feature]>,
    pub(crate) shaper: &'static dyn ComplexShaper,
    pub(crate) shaper_data: Option<ShaperData>,
    pub(crate) map: FeatureMap,
    pub(crate) frac_mask: u32,
    pub(crate) numr_mask: u32,
    pub(crate) dnom_mask: u32,
    pub(crate) rtlm_mask: u32,
    pub(crate) kern_mask: u32,
    pub(crate) requested_kerning: bool,
    pub(crate) has_frac: bool,
    pub(crate) has_gpos_mark: bool,
    pub(crate) zero_marks: bool,
    pub(crate) fallback_glyph_classes: bool,
    pub(crate) fallback_mark_positioning: bool,
    pub(crate) adjust_mark_positioning_when_zeroing: bool,
    pub(crate) apply_gpos: bool,
    pub(crate) apply_kern: bool,
}

impl PlanCore {
    pub(crate) fn shaper_data<T: 'static>(&self) -> Option<&T> {
        self.shaper_data.as_ref()?.downcast_ref()
    }

    pub(crate) fn matches(&self, properties: &SegmentProperties, features: &[Feature]) -> bool {
        self.properties == *properties && *self.features == *features
    }

    fn is_equivalent(&self, other: &Self) -> bool {
        let (left, right) = (&self.properties, &other.properties);
        self.shaper.is_script_neutral()
            && other.shaper.is_script_neutral()
            && self.shaper_data.is_none()
            && other.shaper_data.is_none()
            && left.direction == right.direction
            && left.language == right.language
            && Direction::from_script(left.script) == Direction::from_script(right.script)
            && self.features == other.features
            && self.frac_mask == other.frac_mask
            && self.numr_mask == other.numr_mask
            && self.dnom_mask == other.dnom_mask
            && self.rtlm_mask == other.rtlm_mask
            && self.kern_mask == other.kern_mask
            && self.requested_kerning == other.requested_kerning
            && self.has_frac == other.has_frac
            && self.has_gpos_mark == other.has_gpos_mark
            && self.zero_marks == other.zero_marks
            && self.fallback_glyph_classes == other.fallback_glyph_classes
            && self.fallback_mark_positioning == other.fallback_mark_positioning
            && self.adjust_mark_positioning_when_zeroing
                == other.adjust_mark_positioning_when_zeroing
            && self.apply_gpos == other.apply_gpos
            && self.apply_kern == other.apply_kern
            && self.map.is_equivalent(&other.map)
    }
}

impl fmt::Debug for PlanCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlanCore")
            .field("properties", &self.properties)
            .field("features", &self.features)
            .field("shaper", &self.shaper)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct ShapePlan {
    pub(crate) face: Arc<ShapeFace>,
    pub(crate) core: Arc<PlanCore>,
}

impl ShapePlan {
    pub(crate) fn new(face: Arc<ShapeFace>, core: Arc<PlanCore>) -> Self {
        Self { face, core }
    }

    pub fn properties(&self) -> &SegmentProperties {
        &self.core.properties
    }

    pub fn features(&self) -> &[Feature] {
        &self.core.features
    }

    pub fn face(&self) -> &Arc<ShapeFace> {
        &self.face
    }

    pub fn shape(&self, buffer: &mut Buffer) {
        crate::pipeline::shape::shape(&self.core, &self.face.context, buffer);
    }

    pub fn is_equivalent(&self, other: &ShapePlan) -> bool {
        Arc::ptr_eq(&self.face, &other.face)
            && (Arc::ptr_eq(&self.core, &other.core) || self.core.is_equivalent(&other.core))
    }
}
