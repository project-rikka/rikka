use std::time::{Duration, Instant};

use rikka_test_fonts::Writer;
use rikka_text_font::{Font, GlyphId};
use rikka_text_raster::{GlyphImage, GlyphRequest, Rasterizer};

const UNITS_PER_EM: u16 = 1000;
const ZIGZAG_POINTS: u16 = 60_000;
const COPIES: u16 = 16;
const ON_CURVE: u8 = 1;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
const MORE_COMPONENTS: u16 = 0x0020;
const LONG_LOCA_OFFSETS: u16 = 1;
const HEAD_INDEX_TO_LOC_FORMAT: usize = 50;

fn zigzag() -> Vec<u8> {
    let mut w = Writer::new();
    w.i16(1).i16(0).i16(0).i16(999).i16(1000);
    w.u16(ZIGZAG_POINTS - 1).u16(0);
    for _ in 0..ZIGZAG_POINTS {
        w.u8(ON_CURVE);
    }
    let xs = (0..ZIGZAG_POINTS).map(|index| (i32::from(index) * 7 % 1000) as i16);
    let ys = (0..ZIGZAG_POINTS).map(|index| if index % 2 == 0 { 0 } else { 1000 });
    let mut previous = 0;
    for x in xs {
        w.i16(x - previous);
        previous = x;
    }
    previous = 0;
    for y in ys {
        w.i16(y - previous);
        previous = y;
    }
    w.align(4).finish()
}

fn copies() -> Vec<u8> {
    let mut w = Writer::new();
    w.i16(-1).i16(0).i16(0).i16(999).i16(1000);
    for copy in 0..COPIES {
        let more = if copy + 1 < COPIES {
            MORE_COMPONENTS
        } else {
            0
        };
        w.u16(ARGS_ARE_XY_VALUES | more).u16(1).i8(0).i8(0);
    }
    w.align(4).finish()
}

fn huge_font() -> Vec<u8> {
    let glyphs = [Vec::new(), zigzag(), copies()];
    let mut glyf = Vec::new();
    let mut loca = Writer::new();
    for glyph in &glyphs {
        loca.u32(glyf.len() as u32);
        glyf.extend_from_slice(glyph);
    }
    loca.u32(glyf.len() as u32);
    let mut head = rikka_test_fonts::head(UNITS_PER_EM);
    head[HEAD_INDEX_TO_LOC_FORMAT..HEAD_INDEX_TO_LOC_FORMAT + 2]
        .copy_from_slice(&LONG_LOCA_OFFSETS.to_be_bytes());
    rikka_test_fonts::sfnt(&[
        (b"head", head),
        (b"maxp", rikka_test_fonts::maxp(glyphs.len() as u16)),
        (b"loca", loca.finish()),
        (b"glyf", glyf),
    ])
}

#[test]
fn oversized_outlines_fail_fast() {
    let data = huge_font();
    let font = Font::new(&data).unwrap();
    let mut rasterizer = Rasterizer::new();
    let mut image = GlyphImage::new();
    let started = Instant::now();
    for glyph in [1, 2] {
        let request = GlyphRequest::new(GlyphId(glyph), 64.0).color(false);
        assert!(
            rasterizer
                .render_outline(&font, &request, &mut image)
                .is_err(),
            "glyph {glyph}"
        );
        assert!(
            rasterizer.render(&font, &request, &mut image).is_err(),
            "glyph {glyph}"
        );
    }
    assert!(started.elapsed() < Duration::from_secs(20));
}
