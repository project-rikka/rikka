use rikka_text_font::{Font, GlyphId, Location, Tag};

use crate::icon_files;
use crate::icon_key::IconKey;

const FILL: Tag = Tag::new(b"FILL");
const WEIGHT: Tag = Tag::new(b"wght");
const GRADE: Tag = Tag::new(b"GRAD");
const OPTICAL_SIZE: Tag = Tag::new(b"opsz");

pub(crate) struct IconGlyph {
    pub(crate) font: Font<'static>,
    pub(crate) glyph: GlyphId,
    pub(crate) location: Location,
    pub(crate) pen: (f32, f32),
}

pub(crate) fn icon_glyph(key: IconKey) -> Option<IconGlyph> {
    let font = icon_files::font(key.font)?;
    let glyph = font
        .charmap()
        .and_then(|charmap| charmap.map(key.glyph as u32))
        .filter(|glyph| glyph.0 != 0);
    let Some(glyph) = glyph else {
        tracing::warn!(
            "icon codepoint {:?} has no glyph in {}",
            key.glyph,
            key.font.file()
        );
        return None;
    };
    let fvar = font.fvar();
    let clamped = |tag: Tag, value: f32| {
        fvar?
            .axes()
            .find(|axis| axis.tag == tag)
            .map(|axis| (tag, value.clamp(axis.min, axis.max)))
    };
    let settings = [
        clamped(FILL, f32::from_bits(key.fill_bits)),
        clamped(WEIGHT, f32::from_bits(key.weight_bits)),
        clamped(GRADE, f32::from_bits(key.grade_bits)),
        clamped(OPTICAL_SIZE, key.optical_size()),
    ];
    let location = font.location(settings.into_iter().flatten());
    let size = key.size();
    let em = size / f32::from(font.units_per_em().max(1));
    let advance = font
        .advance_width(glyph, &location)
        .map_or(size, |advance| advance * em);
    let pen = ((size - advance) / 2.0, (0.5 - key.font.middle()) * size);
    Some(IconGlyph {
        font,
        glyph,
        location,
        pen,
    })
}
