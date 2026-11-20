use crate::support::glyph;
use crate::support::pixel;
use crate::support::sfnt::{GLYPH_COUNT, font};
use rikka_test_fonts::Writer;
use rikka_test_fonts::font_bytes;
use rikka_text_font::Font;
use rikka_text_font::GlyphId;
use rikka_text_raster::SvgGlyph;
use rikka_text_raster::{GlyphImage, GlyphImageKind, GlyphRequest, Rasterizer, Rendered};

const SVG: &[u8] = include_bytes!("../testdata/glyph.svg");
const SVG_GZIP: &[u8] = include_bytes!("../testdata/glyph.svg.gz");

fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let row: Vec<u8> = std::iter::once(0)
        .chain(rgba.repeat(width as usize))
        .collect();
    let raw = row.repeat(height as usize);
    let mut zlib = vec![0x78, 0x01, 0x01];
    zlib.extend_from_slice(&(raw.len() as u16).to_le_bytes());
    zlib.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&rikka_deflate::adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8; 4], body: &[u8]| {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        let mut crc = rikka_deflate::Crc32::new();
        crc.update(kind);
        crc.update(body);
        out.extend_from_slice(&crc.finish().to_be_bytes());
    };
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &zlib);
    chunk(b"IEND", &[]);
    out
}

type SbixRecord = (i16, i16, &'static [u8; 4], Vec<u8>);

fn sbix() -> Vec<u8> {
    let mut glyphs: Vec<Option<SbixRecord>> = vec![None; usize::from(GLYPH_COUNT)];
    glyphs[5] = Some((1, -1, b"png ", png(4, 4, [255, 0, 0, 255])));
    glyphs[6] = Some((0, 0, b"jpg ", vec![0xFF, 0xD8, 0xFF]));
    let mut truncated = png(4, 4, [255, 0, 0, 255]);
    truncated.truncate(truncated.len() - 30);
    glyphs[7] = Some((0, 0, b"png ", truncated));
    let mut enormous = png(4, 4, [255, 0, 0, 255]);
    enormous[16..24].copy_from_slice(&[0x7f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff]);
    glyphs[10] = Some((0, 0, b"png ", enormous));
    let mut strike = Writer::new();
    strike.u16(20).u16(72);
    let offsets = strike.len();
    for _ in 0..=GLYPH_COUNT {
        strike.u32(0);
    }
    for (index, glyph) in glyphs.iter().enumerate() {
        let start = strike.len();
        strike.set_u32(offsets + index * 4, start);
        if let Some((x, y, tag, data)) = glyph {
            strike.i16(*x).i16(*y).raw(*tag).raw(data);
        }
    }
    let end = strike.len();
    strike.set_u32(offsets + usize::from(GLYPH_COUNT) * 4, end);
    let mut table = Writer::new();
    table.u16(1).u16(1).u32(1).u32(12).raw(strike.bytes());
    table.finish()
}

fn svg() -> Vec<u8> {
    let mut table = Writer::new();
    table.u16(0).u32(10).u32(0);
    let list = table.len();
    table.u16(2);
    let records = table.len();
    table.u16(7).u16(8).u32(0).u32(SVG.len() as u32);
    table.u16(9).u16(9).u32(0).u32(SVG_GZIP.len() as u32);
    let plain = table.len() - list;
    table.set_u32(records + 4, plain);
    table.raw(SVG);
    let compressed = table.len() - list;
    table.set_u32(records + 16, compressed);
    table.raw(SVG_GZIP);
    table.finish()
}

#[test]
fn sbix_png_glyphs_scale_from_strike() {
    let data = font(&[], vec![(b"sbix", sbix())]);
    let font = Font::new(&data).unwrap();
    let mut image = GlyphImage::new();
    let request = GlyphRequest::new(GlyphId(5), 40.0);
    assert_eq!(
        Rasterizer::new().render(&font, &request, &mut image),
        Ok(Rendered::Image)
    );
    assert_eq!(image.kind(), GlyphImageKind::Color);
    assert_eq!(
        (image.left(), image.top(), image.width(), image.height()),
        (2, 6, 8, 8)
    );
    assert_eq!(pixel(&image, 4, 4), [255, 0, 0, 255]);
    let edge = pixel(&image, 0, 4);
    assert!(edge[3] > 0 && edge[3] < 255 && edge[0] == edge[3]);
    let half = GlyphRequest::new(GlyphId(5), 10.0);
    assert_eq!(
        Rasterizer::new().render(&font, &half, &mut image),
        Ok(Rendered::Image)
    );
    assert_eq!(
        (image.left(), image.top(), image.width(), image.height()),
        (0, 2, 3, 3)
    );
    assert_eq!(pixel(&image, 1, 1), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 0, 1), [128, 0, 0, 128]);
}

#[test]
fn bad_sbix_images_fall_back_to_outlines() {
    let data = font(&[], vec![(b"sbix", sbix())]);
    let font = Font::new(&data).unwrap();
    for glyph in [6, 7, 10] {
        let mut image = GlyphImage::new();
        let request = GlyphRequest::new(GlyphId(glyph), 40.0);
        assert_eq!(
            Rasterizer::new().render(&font, &request, &mut image),
            Ok(Rendered::Image),
            "glyph {glyph}"
        );
        assert_eq!(image.kind(), GlyphImageKind::Mask, "glyph {glyph}");
    }
}

#[test]
fn svg_documents_are_handed_back_decompressed() {
    let data = font(&[], vec![(b"SVG ", svg())]);
    let font = Font::new(&data).unwrap();
    let mut rasterizer = Rasterizer::new();
    let mut image = GlyphImage::new();
    for (glyph, compressed) in [(8u16, false), (9, true)] {
        let request = GlyphRequest::new(GlyphId(glyph), 32.0);
        let expected = SvgGlyph {
            glyph: GlyphId(glyph),
            units_per_em: 1000,
            document: SVG,
        };
        assert_eq!(
            rasterizer.render(&font, &request, &mut image),
            Ok(Rendered::Svg(expected)),
            "compressed {compressed}"
        );
    }
    let mask = GlyphRequest::new(GlyphId(9), 32.0).color(false);
    assert_eq!(
        rasterizer.render(&font, &mask, &mut image),
        Ok(Rendered::Image)
    );
}

#[test]
fn bitmap_emoji_resample_to_requested_size() {
    let data = font_bytes("NotoColorEmoji.ttf");
    let font = Font::new(&data).unwrap();
    let grinning = glyph(&font, '\u{1F600}');
    let strike = font.bitmap_glyph(grinning, 109).unwrap();
    let scale_x = f32::from(strike.metrics.width) / f32::from(strike.ppem_x);
    let scale_y = f32::from(strike.metrics.height) / f32::from(strike.ppem_y);
    let mut rasterizer = Rasterizer::new();
    let mut image = GlyphImage::new();
    for ppem in [12.0f32, 20.0, 32.0, 64.0, 109.0, 160.0] {
        let request = GlyphRequest::new(grinning, ppem);
        assert_eq!(
            rasterizer.render(&font, &request, &mut image),
            Ok(Rendered::Image)
        );
        assert_eq!(image.kind(), GlyphImageKind::Color);
        let expected_width = ppem * scale_x;
        let expected_height = ppem * scale_y;
        assert!(
            (image.width() as f32 - expected_width).abs() <= 2.0,
            "{ppem}: {}",
            image.width()
        );
        assert!(
            (image.height() as f32 - expected_height).abs() <= 2.0,
            "{ppem}: {}",
            image.height()
        );
        let top = f32::from(strike.metrics.bearing_y) * ppem / f32::from(strike.ppem_y);
        assert!(
            (image.top() as f32 - top).abs() <= 1.0,
            "{ppem}: top {}",
            image.top()
        );
        let opaque = image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .count();
        let total = (image.width() * image.height()) as usize;
        assert!(opaque * 2 > total, "{ppem}: {opaque} of {total}");
        assert!(
            image
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[..3].iter().all(|&c| c <= pixel[3]))
        );
        let center = pixel(&image, image.width() / 2, image.height() / 4);
        assert!(
            center[0] > 180 && center[1] > 120 && center[2] < 120,
            "{ppem}: {center:?}"
        );
    }
}
