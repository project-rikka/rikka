use std::hint::black_box;

use divan::Bencher;
use divan::counter::ItemsCount;
use rikka_test_fonts::find_loaded_font;
use rikka_text_font::{Font, FontFile, GlyphId, Location, Tag};
use text_parsers::{FONT_CASES, FontCase, font_case, installed_font_cases};
use text_raster::{
    FreeType, PreparedMasks, RikkaMasks, SwashMasks, raster_glyphs, subpixel_offset,
};

const SIZES: [u32; 5] = [12, 16, 24, 48, 96];

fn main() {
    divan::main();
}

fn static_instances() -> Vec<&'static str> {
    FONT_CASES
        .iter()
        .filter(|case| case.weight.is_none() && find_loaded_font(case.file).is_some())
        .map(|case| case.name)
        .collect()
}

struct Subject {
    case: FontCase,
    data: &'static [u8],
    font: Font<'static>,
    location: Location,
    glyphs: Vec<GlyphId>,
}

impl Subject {
    fn new(name: &str) -> Self {
        let case = font_case(name).expect("font case should exist");
        let data = find_loaded_font(case.file).expect("font should be installed");
        let font = FontFile::new(data)
            .and_then(|file| file.face(case.index))
            .expect("font should parse");
        let location = match case.weight {
            Some(weight) => font.location([(Tag::new(b"wght"), weight)]),
            None => Location::default(),
        };
        let glyphs = raster_glyphs(&font);
        Self {
            case,
            data,
            font,
            location,
            glyphs,
        }
    }
}

trait Rasterizer {
    fn new(subject: &Subject) -> Self;

    fn render(&mut self, subject: &Subject, ppem: f32) -> usize;
}

struct Rikka(RikkaMasks);

impl Rasterizer for Rikka {
    fn new(subject: &Subject) -> Self {
        Self(RikkaMasks::new().location(subject.location.clone()))
    }

    fn render(&mut self, subject: &Subject, ppem: f32) -> usize {
        subject
            .glyphs
            .iter()
            .enumerate()
            .map(|(index, &glyph)| {
                self.0
                    .render(&subject.font, glyph, ppem, subpixel_offset(index))
            })
            .sum()
    }
}

struct RikkaPrepared(PreparedMasks);

impl Rasterizer for RikkaPrepared {
    fn new(subject: &Subject) -> Self {
        Self(PreparedMasks::new(
            &subject.font,
            &subject.glyphs,
            subject.location.clone(),
        ))
    }

    fn render(&mut self, subject: &Subject, ppem: f32) -> usize {
        self.0.render_all(&subject.glyphs, ppem)
    }
}

struct Fontdue(fontdue::Font);

impl Rasterizer for Fontdue {
    fn new(subject: &Subject) -> Self {
        let settings = fontdue::FontSettings {
            collection_index: subject.case.index,
            ..fontdue::FontSettings::default()
        };
        Self(
            fontdue::Font::from_bytes(subject.data, settings)
                .expect("fontdue should read the font"),
        )
    }

    fn render(&mut self, subject: &Subject, ppem: f32) -> usize {
        subject
            .glyphs
            .iter()
            .map(|glyph| self.0.rasterize_indexed(glyph.0, ppem).1.len())
            .sum()
    }
}

struct Swash {
    masks: SwashMasks,
    font: swash::FontRef<'static>,
}

impl Rasterizer for Swash {
    fn new(subject: &Subject) -> Self {
        Self {
            masks: SwashMasks::new().weight(subject.case.weight),
            font: swash::FontRef::from_index(subject.data, subject.case.index as usize)
                .expect("swash should read the font"),
        }
    }

    fn render(&mut self, subject: &Subject, ppem: f32) -> usize {
        let mut pixels = 0;
        self.masks
            .render_all(self.font, &subject.glyphs, ppem, |_, image| {
                pixels += image.data.len();
            });
        pixels
    }
}

struct Freetype(FreeType);

impl Rasterizer for Freetype {
    fn new(subject: &Subject) -> Self {
        Self(
            FreeType::open(subject.data, subject.case.index)
                .expect("FreeType should read the font")
                .weight(subject.case.weight),
        )
    }

    fn render(&mut self, subject: &Subject, ppem: f32) -> usize {
        self.0.render_all(&subject.glyphs, ppem)
    }
}

#[divan::bench(types = [Rikka, Swash, Freetype], consts = SIZES, args = installed_font_cases())]
fn rasterize<R: Rasterizer, const PPEM: u32>(bencher: Bencher, name: &str) {
    let subject = Subject::new(name);
    let mut rasterizer = R::new(&subject);
    bencher
        .counter(ItemsCount::new(subject.glyphs.len()))
        .bench_local(|| rasterizer.render(&subject, black_box(PPEM as f32)));
}

#[divan::bench(types = [RikkaPrepared, Fontdue], consts = SIZES, args = static_instances())]
fn rasterize_prepared<R: Rasterizer, const PPEM: u32>(bencher: Bencher, name: &str) {
    rasterize::<R, PPEM>(bencher, name);
}
