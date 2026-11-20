use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::unicode::space_fallback::SpaceKind;

pub(crate) fn position_fallback_spaces(font: &FontContext, buffer: &mut Buffer) {
    let units_per_em = font.units_per_em();
    for index in 0..buffer.len {
        let info = buffer.info[index];
        if !info.is_unicode_space() || info.ligated() {
            continue;
        }
        let kind = info.space_fallback();
        let position = &mut buffer.pos[index];
        match kind {
            SpaceKind::Em
            | SpaceKind::Em2
            | SpaceKind::Em3
            | SpaceKind::Em4
            | SpaceKind::Em5
            | SpaceKind::Em6
            | SpaceKind::Em16 => {
                let divisor = kind as i32;
                position.x_advance = (units_per_em + divisor / 2) / divisor;
            }
            SpaceKind::FourEm18 => {
                position.x_advance = (i64::from(units_per_em) * 4 / 18) as i32;
            }
            SpaceKind::Figure => {
                if let Some(glyph) =
                    (b'0'..=b'9').find_map(|digit| font.nominal_glyph(u32::from(digit)))
                {
                    position.x_advance = font.advance(u32::from(glyph));
                }
            }
            SpaceKind::Punctuation => {
                if let Some(glyph) = font
                    .nominal_glyph(u32::from(b'.'))
                    .or_else(|| font.nominal_glyph(u32::from(b',')))
                {
                    position.x_advance = font.advance(u32::from(glyph));
                }
            }
            SpaceKind::Narrow => position.x_advance /= 2,
            SpaceKind::NotSpace | SpaceKind::Space => {}
        }
    }
}
