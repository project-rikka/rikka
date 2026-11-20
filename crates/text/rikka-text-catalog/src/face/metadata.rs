use std::fmt;
use std::path::Path;

use rikka_text_font::Tag;

use crate::cjk::CjkRegion;
use crate::coverage::Coverage;
use crate::face::attributes::{Axis, ColorFormats, FaceClass};
use crate::face::record::FaceRecord;
use crate::face::source::FaceSource;
use crate::style::{FontStyle, Weight, Width};

pub struct Face {
    pub(crate) source: FaceSource,
    pub(crate) record: FaceRecord,
    pub(crate) coverage: Coverage,
    pub(crate) folded_families: Box<[String]>,
    pub(crate) class: FaceClass,
    pub(crate) cjk_region: Option<CjkRegion>,
}

impl Face {
    pub fn family(&self) -> &str {
        &self.record.families[0]
    }

    pub fn families(&self) -> impl Iterator<Item = &str> {
        self.record.families.iter().map(|family| &**family)
    }

    pub fn style_name(&self) -> &str {
        &self.record.style_name
    }

    pub fn full_name(&self) -> &str {
        &self.record.full_name
    }

    pub fn path(&self) -> Option<&Path> {
        self.source.path()
    }

    pub fn index(&self) -> u32 {
        self.record.index
    }

    pub fn weight(&self) -> Weight {
        Weight(self.record.weight.default)
    }

    pub fn width(&self) -> Width {
        Width(self.record.width.default)
    }

    pub fn style(&self) -> FontStyle {
        let slope = self.record.slope;
        if slope.upright {
            FontStyle::Normal
        } else if slope.italic {
            FontStyle::Italic
        } else if let Some((angle, _)) = slope.oblique {
            FontStyle::Oblique(angle)
        } else {
            FontStyle::Normal
        }
    }

    pub fn axes(&self) -> &[Axis] {
        &self.record.axes
    }

    pub fn is_variable(&self) -> bool {
        !self.record.axes.is_empty()
    }

    pub fn is_monospace(&self) -> bool {
        self.record.monospace
    }

    pub fn is_color(&self) -> bool {
        self.record.color.any()
    }

    pub fn color_formats(&self) -> ColorFormats {
        self.record.color
    }

    pub fn scripts(&self) -> &[Tag] {
        &self.record.scripts
    }

    #[inline]
    pub fn covers(&self, character: char) -> bool {
        self.coverage.contains(character as u32)
    }

    pub fn ascii_coverage(&self) -> u128 {
        self.coverage.ascii()
    }
    pub fn coverage_count(&self) -> u32 {
        self.coverage.count()
    }
}

impl fmt::Debug for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Face")
            .field("family", &self.family())
            .field("style", &self.style_name())
            .field("path", &self.path())
            .field("index", &self.index())
            .finish_non_exhaustive()
    }
}
