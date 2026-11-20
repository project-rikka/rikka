use crate::error::{Error, Result};
use crate::glyf::emit::emit_contours;
use crate::glyf::loader::Loader;
use crate::outline::bounding_sink::BoundingSink;
use crate::outline::bounds::Bounds;
use crate::outline::scratch::OutlineScratch;
use crate::outline::sink::OutlineSink;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;
use crate::variations::location::Location;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineFormat {
    TrueType,
    Cff,
    Cff2,
}

impl<'a> Font<'a> {
    pub fn outline_format(&self) -> Option<OutlineFormat> {
        if self.has(TableId::Glyf) && self.has(TableId::Loca) {
            Some(OutlineFormat::TrueType)
        } else if self.has(TableId::Cff2) {
            Some(OutlineFormat::Cff2)
        } else if self.has(TableId::Cff) {
            Some(OutlineFormat::Cff)
        } else {
            None
        }
    }

    pub fn outline(
        &self,
        glyph: GlyphId,
        location: &Location,
        scratch: &mut OutlineScratch,
        sink: &mut impl OutlineSink,
    ) -> Result<()> {
        if glyph.0 >= self.glyph_count() {
            return Err(Error::GlyphIndex {
                glyph: glyph.0,
                count: self.glyph_count(),
            });
        }
        match self
            .outline_format()
            .ok_or(Error::MissingTable(Tag::new(b"glyf")))?
        {
            OutlineFormat::TrueType => {
                let mut loader =
                    Loader::new(self, location.coords(), scratch).ok_or(Error::Malformed)?;
                loader.load_outline(glyph).ok_or(Error::Malformed)?;
                emit_contours(&scratch.points, &scratch.flags, &scratch.contour_ends, sink);
            }
            OutlineFormat::Cff | OutlineFormat::Cff2 => {
                crate::cff::outline(
                    self,
                    glyph,
                    location.coords(),
                    scratch.cff.get_or_insert_default(),
                    sink,
                )
                .ok_or(Error::Malformed)?;
            }
        }
        Ok(())
    }

    pub fn glyph_bounds(
        &self,
        glyph: GlyphId,
        location: &Location,
        scratch: &mut OutlineScratch,
    ) -> Option<Bounds> {
        if location.is_default() && self.outline_format() == Some(OutlineFormat::TrueType) {
            return self.glyf()?.header_bounds(glyph);
        }
        self.outline_bounds(glyph, location, scratch)
    }

    pub(crate) fn outline_bounds(
        &self,
        glyph: GlyphId,
        location: &Location,
        scratch: &mut OutlineScratch,
    ) -> Option<Bounds> {
        let mut sink = BoundsOnly;
        let mut bounding = BoundingSink::new(&mut sink);
        self.outline(glyph, location, scratch, &mut bounding).ok()?;
        let bounds = bounding.bounds;
        Some(if bounds.is_empty() {
            Bounds::default()
        } else {
            bounds
        })
    }
}

struct BoundsOnly;

impl OutlineSink for BoundsOnly {
    fn move_to(&mut self, _: f32, _: f32) {}
    fn line_to(&mut self, _: f32, _: f32) {}
    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn cubic_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
    fn close(&mut self) {}
}
