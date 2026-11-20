use std::str::FromStr;

use rikka_test_fonts::Writer;
use rikka_text_font::FontFile;
use rikka_text_shape::{
    Buffer, BufferFlags, Direction, Features, Language, SegmentProperties, ShapeFace,
};
use rikka_text_unicode::Script;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shaped {
    pub(crate) glyph: u16,
    pub(crate) cluster: u32,
    pub(crate) x_advance: i32,
    pub(crate) x_offset: i32,
    pub(crate) y_offset: i32,
    pub(crate) unsafe_to_break: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Request<'a> {
    pub(crate) script: Option<Script>,
    pub(crate) direction: Option<Direction>,
    pub(crate) language: Option<&'a str>,
    pub(crate) features: &'a str,
    pub(crate) flags: BufferFlags,
    pub(crate) face: u32,
    pub(crate) variations: &'a [(&'a [u8; 4], f32)],
}

pub(crate) fn shape_with(data: &'static [u8], text: &str, request: &Request<'_>) -> Vec<Shaped> {
    let file = FontFile::new(data).expect("failed to parse font");
    let font = file.face(request.face).expect("missing face");
    let location = font.location(
        request
            .variations
            .iter()
            .map(|(tag, value)| (rikka_text_font::Tag::new(tag), *value)),
    );
    let face = ShapeFace::new(font, location);
    let mut buffer = Buffer::new();
    buffer.push_str(text, 0);
    buffer.set_flags(request.flags);
    let mut properties = match request.script {
        Some(script) => SegmentProperties::new(script),
        None => buffer.guess_properties(),
    };
    if let Some(direction) = request.direction {
        properties.direction = direction;
    }
    properties.language = request
        .language
        .map(|language| Language::from_str(language).expect("invalid language"));
    let features = Features::from_str(request.features).expect("invalid features");
    face.plan(&properties, &features).shape(&mut buffer);
    buffer
        .glyph_infos()
        .iter()
        .zip(buffer.glyph_positions())
        .map(|(info, position)| Shaped {
            glyph: info.glyph().0,
            cluster: info.cluster(),
            x_advance: position.x_advance,
            x_offset: position.x_offset,
            y_offset: position.y_offset,
            unsafe_to_break: info.unsafe_to_break(),
        })
        .collect()
}

pub(crate) fn shape_text(data: &'static [u8], text: &str) -> Vec<Shaped> {
    shape_with(data, text, &Request::default())
}

pub(crate) fn glyphs(shaped: &[Shaped]) -> Vec<u16> {
    shaped.iter().map(|glyph| glyph.glyph).collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SyntheticFont {
    mappings: Vec<(u32, u16)>,
    advances: Vec<u16>,
    kerning: Vec<(u16, u16, i16)>,
}

impl SyntheticFont {
    pub(crate) fn new(glyph_count: u16) -> Self {
        Self {
            mappings: Vec::new(),
            advances: vec![500; usize::from(glyph_count)],
            kerning: Vec::new(),
        }
    }

    pub(crate) fn map(mut self, codepoint: u32, glyph: u16) -> Self {
        self.mappings.push((codepoint, glyph));
        self
    }

    pub(crate) fn kern(mut self, left: u16, right: u16, value: i16) -> Self {
        self.kerning.push((left, right, value));
        self
    }

    fn cmap(&self) -> Vec<u8> {
        let mut mappings = self.mappings.clone();
        mappings.sort_unstable();
        let mut subtable = Writer::new();
        subtable
            .u16(12)
            .u16(0)
            .u32(16 + 12 * mappings.len() as u32)
            .u32(0)
            .u32(mappings.len() as u32);
        for (codepoint, glyph) in &mappings {
            subtable
                .u32(*codepoint)
                .u32(*codepoint)
                .u32(u32::from(*glyph));
        }
        Writer::new()
            .u16(0)
            .u16(1)
            .u16(3)
            .u16(10)
            .u32(12)
            .raw(&subtable.finish())
            .finish()
    }

    fn kern_table(&self) -> Vec<u8> {
        let mut pairs = self.kerning.clone();
        pairs.sort_unstable();
        let mut subtable = Writer::new();
        subtable
            .u16(0)
            .u16(14 + 6 * pairs.len() as u16)
            .u16(0x0001)
            .u16(pairs.len() as u16)
            .u16(6)
            .u16(0)
            .u16(0);
        for (left, right, value) in &pairs {
            subtable.u16(*left).u16(*right).i16(*value);
        }
        Writer::new().u16(0).u16(1).raw(&subtable.finish()).finish()
    }

    pub(crate) fn build(&self) -> &'static [u8] {
        let glyph_count = self.advances.len() as u16;
        let mut hhea = Writer::new();
        hhea.u32(0x0001_0000).i16(800).i16(-200).i16(0).u16(1000);
        hhea.raw(&[0; 22]).u16(glyph_count);
        let mut hmtx = Writer::new();
        for advance in &self.advances {
            hmtx.u16(*advance).i16(0);
        }
        let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
            (b"cmap", self.cmap()),
            (b"head", rikka_test_fonts::head(1000)),
            (b"hhea", hhea.finish()),
            (b"hmtx", hmtx.finish()),
            (b"maxp", rikka_test_fonts::maxp(glyph_count)),
        ];
        if !self.kerning.is_empty() {
            tables.push((b"kern", self.kern_table()));
        }
        rikka_test_fonts::sfnt(&tables).leak()
    }
}

pub(crate) fn nominal_glyph(data: &[u8], codepoint: u32) -> u16 {
    let file = FontFile::new(data).expect("failed to parse font");
    let font = file.face(0).expect("missing face");
    font.charmap()
        .and_then(|charmap| charmap.map(codepoint))
        .map(|glyph| glyph.0)
        .expect("codepoint is not mapped")
}
