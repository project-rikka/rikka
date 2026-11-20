use crate::glyf::loca::Glyf;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::tables::long_metrics::LongMetrics;
use crate::variations::gvar::Gvar;
use crate::variations::location::Location;
use crate::variations::metrics_variations::MetricsVariations;
use rikka_types::Point;

#[derive(Clone, Copy, Debug)]
pub struct GlyphMetrics<'a, 'l> {
    coords: &'l [F2Dot14],
    hmtx: Option<LongMetrics<'a>>,
    hvar: Option<MetricsVariations<'a>>,
    vmtx: Option<LongMetrics<'a>>,
    vvar: Option<MetricsVariations<'a>>,
    gvar: Option<Gvar<'a>>,
    glyf: Option<Glyf<'a>>,
    default_vertical_advance: f32,
    glyph_count: u16,
}

impl<'a, 'l> GlyphMetrics<'a, 'l> {
    pub(crate) fn new(font: &Font<'a>, location: &'l Location) -> Self {
        let coords = location.coords();
        let varied = !coords.is_empty();
        let default_vertical_advance = font.hhea().map_or(f32::from(font.units_per_em()), |hhea| {
            f32::from(hhea.ascender()) - f32::from(hhea.descender())
        });
        Self {
            coords,
            hmtx: font.hmtx(),
            hvar: if varied { font.hvar() } else { None },
            vmtx: font.vmtx(),
            vvar: if varied { font.vvar() } else { None },
            gvar: if varied { font.gvar() } else { None },
            glyf: font.glyf(),
            default_vertical_advance,
            glyph_count: font.glyph_count(),
        }
    }

    #[inline]
    pub fn advance_width(&self, glyph: GlyphId) -> Option<f32> {
        let advance = f32::from(self.hmtx?.advance(glyph)?);
        if let Some(hvar) = self.hvar {
            return Some(advance + hvar.advance_delta(glyph, self.coords));
        }
        Some(advance + self.phantom_deltas(glyph).map_or(0.0, |d| d[1].x - d[0].x))
    }

    #[inline]
    pub fn left_side_bearing(&self, glyph: GlyphId) -> Option<f32> {
        let bearing = f32::from(self.hmtx?.side_bearing(glyph)?);
        let delta = self
            .hvar
            .and_then(|hvar| hvar.leading_bearing_delta(glyph, self.coords))
            .unwrap_or(0.0);
        Some(bearing + delta)
    }

    #[inline]
    pub fn advance_height(&self, glyph: GlyphId) -> Option<f32> {
        if glyph.0 >= self.glyph_count {
            return None;
        }
        let Some(advance) = self.vmtx.and_then(|vmtx| vmtx.advance(glyph)) else {
            return Some(self.default_vertical_advance);
        };
        let advance = f32::from(advance);
        if let Some(vvar) = self.vvar {
            return Some(advance + vvar.advance_delta(glyph, self.coords));
        }
        Some(advance - self.phantom_deltas(glyph).map_or(0.0, |d| d[3].y - d[2].y))
    }

    #[inline]
    pub fn top_side_bearing(&self, glyph: GlyphId) -> Option<f32> {
        let bearing = f32::from(self.vmtx?.side_bearing(glyph)?);
        let delta = self
            .vvar
            .and_then(|vvar| vvar.leading_bearing_delta(glyph, self.coords))
            .unwrap_or(0.0);
        Some(bearing + delta)
    }

    fn phantom_deltas(&self, glyph: GlyphId) -> Option<[Point; 4]> {
        let tuples = self.gvar?.tuples(glyph, self.coords)?;
        let start = self.glyf?.phantom_start(glyph)?;
        let mut total = [Point::default(); 4];
        for tuple in tuples {
            if let Some(deltas) = tuple.phantom_deltas(start) {
                for (sum, delta) in total.iter_mut().zip(deltas) {
                    *sum += delta;
                }
            }
        }
        Some(total)
    }
}

impl<'a> Font<'a> {
    pub fn glyph_metrics<'l>(&self, location: &'l Location) -> GlyphMetrics<'a, 'l> {
        GlyphMetrics::new(self, location)
    }

    pub fn advance_width(&self, glyph: GlyphId, location: &Location) -> Option<f32> {
        self.glyph_metrics(location).advance_width(glyph)
    }
}
