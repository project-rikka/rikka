use rikka_test_fonts::find_loaded_font;
use rikka_text_font::{Font, FontFile, GlyphId, Location, OutlineScratch, OutlineSink, Tag};
use skrifa::instance::{Location as SkrifaLocation, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{MetadataProvider, OutlineGlyphCollection};

const IDEOGRAPHS: u32 = 400;

#[derive(Clone, Copy, Debug)]
pub struct FontCase {
    pub name: &'static str,
    pub file: &'static str,
    pub index: u32,
    pub weight: Option<f32>,
}

pub const FONT_CASES: [FontCase; 3] = [
    FontCase {
        name: "inter",
        file: "InterVariable.ttf",
        index: 0,
        weight: None,
    },
    FontCase {
        name: "inter-700",
        file: "InterVariable.ttf",
        index: 0,
        weight: Some(700.0),
    },
    FontCase {
        name: "noto-sans-cjk",
        file: "NotoSansCJK-VF.otf.ttc",
        index: 0,
        weight: None,
    },
];

pub fn font_case(name: &str) -> Option<FontCase> {
    FONT_CASES.into_iter().find(|case| case.name == name)
}

pub fn installed_font_cases() -> Vec<&'static str> {
    FONT_CASES
        .iter()
        .filter(|case| find_loaded_font(case.file).is_some())
        .map(|case| case.name)
        .collect()
}

pub fn codepoints() -> Vec<u32> {
    (0x21u32..0x7F).chain(0x4E00..0x4E00 + IDEOGRAPHS).collect()
}

#[derive(Debug, Default)]
pub struct Segments(pub usize);

impl OutlineSink for Segments {
    fn move_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn line_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn cubic_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn close(&mut self) {}
}

impl OutlinePen for Segments {
    fn move_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn line_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn curve_to(&mut self, _cx0: f32, _cy0: f32, _cx1: f32, _cy1: f32, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn close(&mut self) {}
}

impl ttf_parser::OutlineBuilder for Segments {
    fn move_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn line_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {
        self.0 += 1;
    }

    fn close(&mut self) {}
}

#[derive(Debug)]
pub struct RikkaParser {
    font: Font<'static>,
    location: Location,
    scratch: OutlineScratch,
}

impl RikkaParser {
    pub fn open(data: &'static [u8], case: &FontCase) -> Option<Self> {
        let font = FontFile::new(data).ok()?.face(case.index).ok()?;
        let location = match case.weight {
            Some(weight) => font.location([(Tag::new(b"wght"), weight)]),
            None => Location::default(),
        };
        Some(Self {
            font,
            location,
            scratch: OutlineScratch::default(),
        })
    }

    pub fn parse(data: &'static [u8], index: u32) -> usize {
        FontFile::new(data)
            .and_then(|file| file.face(index))
            .ok()
            .and_then(|font| font.charmap().map(|_| usize::from(font.glyph_count())))
            .unwrap_or(0)
    }

    pub fn glyphs(&self, codepoints: &[u32]) -> Vec<GlyphId> {
        let Some(charmap) = self.font.charmap() else {
            return Vec::new();
        };
        codepoints
            .iter()
            .filter_map(|&code| charmap.map(code))
            .collect()
    }

    pub fn outlines(&mut self, glyphs: &[GlyphId]) -> usize {
        let mut segments = Segments::default();
        for &glyph in glyphs {
            self.font
                .outline(glyph, &self.location, &mut self.scratch, &mut segments)
                .expect("benchmark glyphs have outlines");
        }
        segments.0
    }

    pub fn advances(&self, glyphs: &[GlyphId]) -> Vec<f32> {
        let metrics = self.font.glyph_metrics(&self.location);
        glyphs
            .iter()
            .map(|&glyph| metrics.advance_width(glyph).unwrap_or(0.0))
            .collect()
    }
}

pub struct SkrifaParser {
    font: skrifa::FontRef<'static>,
    outlines: OutlineGlyphCollection<'static>,
    location: SkrifaLocation,
}

impl SkrifaParser {
    pub fn open(data: &'static [u8], case: &FontCase) -> Option<Self> {
        let font = skrifa::FontRef::from_index(data, case.index).ok()?;
        let location = font
            .axes()
            .location(case.weight.map(|weight| ("wght", weight)));
        Some(Self {
            outlines: font.outline_glyphs(),
            font,
            location,
        })
    }

    pub fn parse(data: &'static [u8], index: u32) -> usize {
        skrifa::FontRef::from_index(data, index)
            .ok()
            .and_then(|font| {
                font.charmap();
                font.maxp().ok()
            })
            .map_or(0, |maxp| usize::from(maxp.num_glyphs()))
    }

    pub fn glyphs(&self, codepoints: &[u32]) -> Vec<skrifa::GlyphId> {
        let charmap = self.font.charmap();
        codepoints
            .iter()
            .filter_map(|&code| charmap.map(code))
            .collect()
    }

    pub fn outlines(&self, glyphs: &[skrifa::GlyphId]) -> usize {
        let mut segments = Segments::default();
        for &glyph in glyphs {
            if let Some(outline) = self.outlines.get(glyph) {
                let settings = DrawSettings::unhinted(Size::unscaled(), &self.location);
                let _ = outline.draw(settings, &mut segments);
            }
        }
        segments.0
    }

    pub fn advances(&self, glyphs: &[skrifa::GlyphId]) -> Vec<f32> {
        let metrics = self.font.glyph_metrics(Size::unscaled(), &self.location);
        glyphs
            .iter()
            .map(|&glyph| metrics.advance_width(glyph).unwrap_or(0.0))
            .collect()
    }
}

pub struct TtfParser {
    face: ttf_parser::Face<'static>,
}

impl TtfParser {
    pub fn open(data: &'static [u8], case: &FontCase) -> Option<Self> {
        let mut face = ttf_parser::Face::parse(data, case.index).ok()?;
        if let Some(weight) = case.weight {
            face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), weight)?;
        }
        Some(Self { face })
    }

    pub fn parse(data: &'static [u8], index: u32) -> usize {
        ttf_parser::Face::parse(data, index)
            .map(|face| usize::from(face.number_of_glyphs()))
            .unwrap_or(0)
    }

    pub fn glyphs(&self, codepoints: &[u32]) -> Vec<ttf_parser::GlyphId> {
        codepoints
            .iter()
            .filter_map(|&code| char::from_u32(code))
            .filter_map(|character| self.face.glyph_index(character))
            .collect()
    }

    pub fn outlines(&self, glyphs: &[ttf_parser::GlyphId]) -> usize {
        let mut segments = Segments::default();
        for &glyph in glyphs {
            self.face.outline_glyph(glyph, &mut segments);
        }
        segments.0
    }

    pub fn advances(&self, glyphs: &[ttf_parser::GlyphId]) -> Vec<f32> {
        glyphs
            .iter()
            .map(|&glyph| self.face.glyph_hor_advance(glyph).map_or(0.0, f32::from))
            .collect()
    }
}

impl std::fmt::Debug for SkrifaParser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkrifaParser")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for TtfParser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("TtfParser").finish_non_exhaustive()
    }
}
