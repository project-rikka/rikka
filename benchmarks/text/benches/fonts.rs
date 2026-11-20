use std::hint::black_box;

use divan::Bencher;
use divan::counter::ItemsCount;
use rikka_test_fonts::find_loaded_font;
use rikka_text_font::FontFile;
use text_parsers::{
    FontCase, RikkaParser, SkrifaParser, TtfParser, codepoints, font_case, installed_font_cases,
};
use text_raster::{FreeType, raster_glyphs};

fn main() {
    divan::main();
}

fn subject(name: &str) -> (FontCase, &'static [u8]) {
    let case = font_case(name).expect("font case should exist");
    (
        case,
        find_loaded_font(case.file).expect("font should be installed"),
    )
}

trait Parser: Sized {
    type Glyph: Copy;

    fn open(data: &'static [u8], case: &FontCase) -> Self;

    fn parse(data: &'static [u8], index: u32) -> usize;

    fn glyphs(&self, codepoints: &[u32]) -> Vec<Self::Glyph>;

    fn outlines(&mut self, glyphs: &[Self::Glyph]) -> usize;

    fn advances(&self, glyphs: &[Self::Glyph]) -> Vec<f32>;
}

struct Rikka(RikkaParser);

impl Parser for Rikka {
    type Glyph = rikka_text_font::GlyphId;

    fn open(data: &'static [u8], case: &FontCase) -> Self {
        Self(RikkaParser::open(data, case).expect("rikka-text-font should read the font"))
    }

    fn parse(data: &'static [u8], index: u32) -> usize {
        RikkaParser::parse(data, index)
    }

    fn glyphs(&self, codepoints: &[u32]) -> Vec<Self::Glyph> {
        self.0.glyphs(codepoints)
    }

    fn outlines(&mut self, glyphs: &[Self::Glyph]) -> usize {
        self.0.outlines(glyphs)
    }

    fn advances(&self, glyphs: &[Self::Glyph]) -> Vec<f32> {
        self.0.advances(glyphs)
    }
}

struct Skrifa(SkrifaParser);

impl Parser for Skrifa {
    type Glyph = skrifa::GlyphId;

    fn open(data: &'static [u8], case: &FontCase) -> Self {
        Self(SkrifaParser::open(data, case).expect("skrifa should read the font"))
    }

    fn parse(data: &'static [u8], index: u32) -> usize {
        SkrifaParser::parse(data, index)
    }

    fn glyphs(&self, codepoints: &[u32]) -> Vec<Self::Glyph> {
        self.0.glyphs(codepoints)
    }

    fn outlines(&mut self, glyphs: &[Self::Glyph]) -> usize {
        self.0.outlines(glyphs)
    }

    fn advances(&self, glyphs: &[Self::Glyph]) -> Vec<f32> {
        self.0.advances(glyphs)
    }
}

struct Ttf(TtfParser);

impl Parser for Ttf {
    type Glyph = ttf_parser::GlyphId;

    fn open(data: &'static [u8], case: &FontCase) -> Self {
        Self(TtfParser::open(data, case).expect("ttf-parser should read the font"))
    }

    fn parse(data: &'static [u8], index: u32) -> usize {
        TtfParser::parse(data, index)
    }

    fn glyphs(&self, codepoints: &[u32]) -> Vec<Self::Glyph> {
        self.0.glyphs(codepoints)
    }

    fn outlines(&mut self, glyphs: &[Self::Glyph]) -> usize {
        self.0.outlines(glyphs)
    }

    fn advances(&self, glyphs: &[Self::Glyph]) -> Vec<f32> {
        self.0.advances(glyphs)
    }
}

#[divan::bench(types = [Rikka, Skrifa, Ttf], args = installed_font_cases())]
fn parse<P: Parser>(bencher: Bencher, name: &str) {
    let (case, data) = subject(name);
    bencher.bench_local(|| P::parse(black_box(data), case.index));
}

#[divan::bench(args = installed_font_cases())]
fn parse_freetype(bencher: Bencher, name: &str) {
    let (case, data) = subject(name);
    bencher.bench_local(|| FreeType::open(black_box(data), case.index).is_some());
}

#[divan::bench(types = [Rikka, Skrifa, Ttf], args = installed_font_cases())]
fn map_characters<P: Parser>(bencher: Bencher, name: &str) {
    let (case, data) = subject(name);
    let parser = P::open(data, &case);
    let codepoints = codepoints();
    bencher
        .counter(ItemsCount::new(codepoints.len()))
        .bench_local(|| parser.glyphs(black_box(&codepoints)).len());
}

#[divan::bench(types = [Rikka, Skrifa, Ttf], args = installed_font_cases())]
fn outlines<P: Parser>(bencher: Bencher, name: &str) {
    let (case, data) = subject(name);
    let mut parser = P::open(data, &case);
    let glyphs = parser.glyphs(&codepoints());
    bencher
        .counter(ItemsCount::new(glyphs.len()))
        .bench_local(|| parser.outlines(black_box(&glyphs)));
}

#[divan::bench(args = installed_font_cases())]
fn outlines_freetype(bencher: Bencher, name: &str) {
    let (case, data) = subject(name);
    let font = FontFile::new(data)
        .and_then(|file| file.face(case.index))
        .expect("font should parse");
    let glyphs = raster_glyphs(&font);
    let mut freetype = FreeType::open(data, case.index)
        .expect("FreeType should read the font")
        .weight(case.weight);
    bencher
        .counter(ItemsCount::new(glyphs.len()))
        .bench_local(|| {
            glyphs
                .iter()
                .map(|&glyph| freetype.outline_segments(glyph))
                .sum::<usize>()
        });
}

#[divan::bench(types = [Rikka, Skrifa, Ttf], args = installed_font_cases())]
fn advances<P: Parser>(bencher: Bencher, name: &str) {
    let (case, data) = subject(name);
    let parser = P::open(data, &case);
    let glyphs = parser.glyphs(&codepoints());
    bencher
        .counter(ItemsCount::new(glyphs.len()))
        .bench_local(|| parser.advances(black_box(&glyphs)).len());
}
