use crate::glyf::glyph::{Glyph, header_bounds};
use crate::outline::bounds::Bounds;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

const METRICS_RECURSION_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Glyf<'a> {
    loca: &'a [u8],
    glyf: &'a [u8],
    long_offsets: bool,
}

impl<'a> Glyf<'a> {
    #[inline]
    fn offset(&self, index: usize) -> Option<usize> {
        if self.long_offsets {
            self.loca.u32_at(index * 4).map(|offset| offset as usize)
        } else {
            self.loca
                .u16_at(index * 2)
                .map(|offset| usize::from(offset) * 2)
        }
    }

    #[inline]
    pub(crate) fn glyph_data(&self, glyph: GlyphId) -> Option<&'a [u8]> {
        let start = self.offset(glyph.index())?;
        let end = self.offset(glyph.index() + 1)?;
        if end < start {
            return None;
        }
        self.glyf.range(start, end - start)
    }
}

impl<'a> Font<'a> {
    pub(crate) fn glyf(&self) -> Option<Glyf<'a>> {
        let long_offsets = self.head()?.index_to_loc_format() != 0;
        Some(Glyf {
            loca: self.table_data(TableId::Loca)?,
            glyf: self.table_data(TableId::Glyf)?,
            long_offsets,
        })
    }
}

impl Glyf<'_> {
    pub(crate) fn phantom_start(&self, glyph: GlyphId) -> Option<usize> {
        match Glyph::parse(self.glyph_data(glyph)?)? {
            Glyph::Empty => Some(0),
            Glyph::Simple(simple) => Some(simple.point_count),
            Glyph::Composite(composite) => Some(composite.components().count()),
        }
    }

    pub(crate) fn metrics_source(&self, glyph: GlyphId) -> Option<(GlyphId, usize)> {
        let mut glyph = glyph;
        for _ in 0..=METRICS_RECURSION_LIMIT {
            let composite = match Glyph::parse(self.glyph_data(glyph)?)? {
                Glyph::Empty => return Some((glyph, 0)),
                Glyph::Simple(simple) => return Some((glyph, simple.point_count)),
                Glyph::Composite(composite) => composite,
            };
            let mut count = 0;
            let mut metrics = None;
            for component in composite.components() {
                count += 1;
                if component.use_my_metrics {
                    metrics = Some(component.glyph);
                }
            }
            match metrics {
                Some(source) => glyph = source,
                None => return Some((glyph, count)),
            }
        }
        None
    }

    pub(crate) fn header_bounds(&self, glyph: GlyphId) -> Option<Bounds> {
        let data = self.glyph_data(glyph)?;
        if data.is_empty() {
            return Some(Bounds::default());
        }
        header_bounds(data)
    }
}
