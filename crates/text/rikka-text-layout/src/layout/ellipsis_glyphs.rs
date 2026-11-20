use rikka_text_unicode::Script;

use crate::fonts::{EllipsisGlyph, EllipsisGlyphs, FaceInstance};
use crate::shaping::{Context, GlyphColumns, RunRequest, shape_run};
use crate::style::Caps;

const ELLIPSIS_TEXTS: [&str; 2] = ["\u{2026}", "..."];

pub(crate) fn ellipsis_of(face: &FaceInstance) -> Option<&EllipsisGlyphs> {
    face.ellipsis.get_or_init(|| shape_ellipsis(face)).as_ref()
}

fn shape_ellipsis(face: &FaceInstance) -> Option<EllipsisGlyphs> {
    let text = ELLIPSIS_TEXTS
        .into_iter()
        .find(|text| text.chars().all(|character| face.maps(character)))?;
    let mut columns = GlyphColumns::default();
    shape_run(
        &RunRequest {
            text,
            range: 0..text.len(),
            face,
            script: Script::Common,
            rtl: false,
            context: Context::FULL,
            caps: Caps::Normal,
        },
        &mut columns,
    );
    let glyphs: Box<[EllipsisGlyph]> = (0..columns.len())
        .map(|index| EllipsisGlyph {
            id: columns.ids[index],
            advance: columns.advances[index],
            x_offset: columns.x_offsets[index],
            y_offset: columns.y_offsets[index],
        })
        .collect();
    let advance = glyphs.iter().map(|glyph| glyph.advance).sum();
    Some(EllipsisGlyphs {
        text,
        glyphs,
        advance,
    })
}
