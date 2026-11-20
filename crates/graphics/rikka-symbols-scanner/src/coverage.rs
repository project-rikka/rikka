use std::path::Path;

use rikka_text_font::{Charmap, Font, GlyphId};

use crate::error::{Error, Result};
use crate::listing::Icon;

pub(crate) fn open_font<'a>(path: &Path, bytes: &'a [u8]) -> Result<Font<'a>> {
    Font::new(bytes).map_err(|source| Error::Font {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn glyph(charmap: &Charmap<'_>, icon: &Icon) -> Option<GlyphId> {
    charmap
        .map(u32::from(icon.codepoint))
        .filter(|glyph| *glyph != GlyphId::NOTDEF)
}

pub(crate) fn check_coverage(path: &Path, bytes: &[u8], icons: &[Icon]) -> Result<()> {
    let charmap = open_font(path, bytes)?.charmap();
    let missing: Vec<String> = icons
        .iter()
        .filter(|icon| {
            charmap
                .as_ref()
                .and_then(|charmap| glyph(charmap, icon))
                .is_none()
        })
        .map(|icon| format!("{} (U+{:04X})", icon.name, u32::from(icon.codepoint)))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::MissingGlyphs {
            path: path.to_owned(),
            missing,
        })
    }
}
