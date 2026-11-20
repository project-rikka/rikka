use std::collections::VecDeque;

use rikka_svg::{ColorScheme, Loader, Svg};
use rikka_text_catalog::FaceId;
use rikka_text_font::{Font, GlyphId};

const CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SvgDocumentKey {
    pub(crate) face: FaceId,
    pub(crate) first_glyph: u16,
    pub(crate) scheme: ColorScheme,
}

impl SvgDocumentKey {
    pub(crate) fn for_glyph(
        font: &Font<'_>,
        face: FaceId,
        glyph: GlyphId,
        scheme: ColorScheme,
    ) -> Option<Self> {
        Some(Self {
            face,
            first_glyph: font.svg_document(glyph)?.first_glyph.0,
            scheme,
        })
    }
}

#[derive(Default)]
pub(crate) struct SvgDocuments {
    recent: VecDeque<(SvgDocumentKey, Option<Svg>)>,
}

impl SvgDocuments {
    pub(crate) fn cached(&mut self, key: SvgDocumentKey) -> Option<Option<&Svg>> {
        let at = self.recent.iter().position(|(cached, _)| *cached == key)?;
        let entry = self.recent.remove(at)?;
        self.recent.push_front(entry);
        Some(self.recent[0].1.as_ref())
    }

    pub(crate) fn parsed(&mut self, key: SvgDocumentKey, document: &[u8]) -> Option<&Svg> {
        if self.cached(key).is_some() {
            return self.recent[0].1.as_ref();
        }
        let parsed = Loader::default()
            .color_scheme(key.scheme)
            .from_bytes(document)
            .inspect_err(|error| tracing::warn!("failed to parse SVG glyph document: {error}"))
            .ok();
        self.recent.truncate(CAPACITY - 1);
        self.recent.push_front((key, parsed));
        self.recent[0].1.as_ref()
    }
}
