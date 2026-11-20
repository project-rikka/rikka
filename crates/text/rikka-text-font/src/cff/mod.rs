mod active_regions;
mod blend;
mod cache;
pub(crate) mod cff1;
mod cff2;
mod charset;
mod charstring;
mod dict;
mod fd_select;
mod index;
mod operands;
mod pen;
mod standard_encoding;
mod standard_strings;
mod subfont;

pub(crate) use cache::CffCache;

use crate::cff::cache::Layout;
use crate::cff::cff1::Cff;
use crate::cff::cff2::Cff2;
use crate::outline::sink::OutlineSink;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;

pub(crate) fn outline(
    font: &Font<'_>,
    glyph: GlyphId,
    coords: &[F2Dot14],
    cache: &mut CffCache,
    sink: &mut impl OutlineSink,
) -> Option<()> {
    let resolved = cache.resolve(font)?;
    let units_per_em = font.units_per_em();
    let (table, subfonts, operands) = (resolved.table, resolved.subfonts, resolved.operands);
    match resolved.layout {
        Layout::Cff(layout) => {
            Cff::bind(table, &layout, units_per_em)?.outline(glyph, subfonts, operands, sink)
        }
        Layout::Cff2(layout) => Cff2::bind(table, &layout, units_per_em)?
            .outline(glyph, coords, subfonts, operands, sink),
    }
}
