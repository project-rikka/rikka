use rikka_deflate::Inflater;
use rikka_text_font::{Font, GlyphId};

const MAX_DOCUMENT: usize = 32 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvgGlyph<'r> {
    pub glyph: GlyphId,
    pub units_per_em: u16,
    pub document: &'r [u8],
}

#[derive(Debug, Default)]
pub(crate) struct SvgExtractor {
    inflater: Inflater,
    document: Vec<u8>,
}

impl SvgExtractor {
    pub(crate) fn extract<'r, 'f: 'r>(
        &'r mut self,
        font: &Font<'f>,
        glyph: GlyphId,
    ) -> Option<SvgGlyph<'r>> {
        let source = font.svg_document(glyph)?;
        let document: &'r [u8] = if source.is_compressed() {
            self.document.clear();
            self.inflater
                .gzip(source.data, &mut self.document, MAX_DOCUMENT)
                .ok()?;
            &self.document
        } else {
            source.data
        };
        Some(SvgGlyph {
            glyph,
            units_per_em: font.units_per_em(),
            document,
        })
    }
}
