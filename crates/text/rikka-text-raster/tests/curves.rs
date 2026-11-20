use crate::support::glyph;
use rikka_test_fonts::font_bytes;
use rikka_text_font::{Font, Location, OutlineScratch};
use rikka_text_raster::{
    CurveBlob, CurveHeader, GlyphImage, GlyphRequest, Rasterizer, coverage_at,
};

fn encode(font: &Font<'_>, request: &GlyphRequest<'_>) -> (CurveBlob, CurveHeader) {
    let mut blob = CurveBlob::new();
    let header = Rasterizer::new()
        .encode_curves(font, request, &mut blob)
        .expect("failed to encode glyph curves");
    (blob, header)
}

fn unorm(word: u32, shift: u32) -> f32 {
    ((word >> shift) & 0xFFFF) as f32 / CurveBlob::UNORM_MAX
}

#[test]
fn curve_blob_layout_matches_spec() {
    let data = font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let id = glyph(&font, '@');
    let (blob, header) = encode(&font, &GlyphRequest::new(id, 1.0));
    let words = blob.words();
    assert_eq!(CurveHeader::read(words), Some(header));
    let bounds = font
        .glyph_bounds(id, &Location::default(), &mut OutlineScratch::new())
        .unwrap();
    let em = f32::from(font.units_per_em());
    assert!((header.bounds.x_min - bounds.x_min / em).abs() < 1e-3);
    assert!((header.bounds.y_max - bounds.y_max / em).abs() < 1e-3);
    let counts = words[CurveBlob::COUNTS_WORD];
    assert_eq!((counts & 0xFFFF) as u16, header.curve_count);
    let bands = usize::from(header.horizontal_bands);
    assert!(bands > 1 && usize::from(header.vertical_bands) == bands);
    let curves = CurveBlob::BAND_HEADERS_WORD + 2 * bands;
    let curves_end = curves + usize::from(header.curve_count) * CurveBlob::CURVE_WORDS;
    let width = header.bounds.x_max - header.bounds.x_min;
    for band in 0..2 * bands {
        let band_word = words[CurveBlob::BAND_HEADERS_WORD + band];
        let offset = (band_word & CurveBlob::BAND_OFFSET_MASK) as usize;
        let count = (band_word >> CurveBlob::BAND_COUNT_SHIFT) as usize;
        assert!(count > 0 && offset >= curves_end);
        let mut previous = f32::INFINITY;
        for index in 0..count {
            let reference = words[offset + index / 2] >> (16 * (index % 2)) & 0xFFFF;
            let curve = reference as usize;
            assert!(curve >= curves && curve < curves_end && (curve - curves).is_multiple_of(3));
            if band < bands {
                let max_x = (0..3)
                    .map(|point| unorm(words[curve + point], 0))
                    .fold(f32::NEG_INFINITY, f32::max)
                    * width;
                assert!(max_x <= previous);
                previous = max_x;
            }
        }
    }
}

fn compare(font: &Font<'_>, request: GlyphRequest<'_>, subpixel: f32) -> (f64, f64) {
    let request = &request.subpixel_x(subpixel);
    let mut rasterizer = Rasterizer::new();
    let mut image = GlyphImage::new();
    assert_eq!(rasterizer.render_outline(font, request, &mut image), Ok(()));
    let mut blob = CurveBlob::new();
    rasterizer.encode_curves(font, request, &mut blob).unwrap();
    let ppem = request.ppem();
    let mut total = 0.0f64;
    let mut large = 0usize;
    let pixels = (image.width() * image.height()) as usize;
    for y in 0..image.height() {
        for (x, &value) in image.row(y).iter().enumerate() {
            let px = (image.left() as f32 + x as f32 + 0.5 - subpixel) / ppem;
            let py = (image.top() as f32 - y as f32 - 0.5) / ppem;
            let expected = f32::from(value) / 255.0;
            let error = f64::from((coverage_at(blob.words(), px, py, ppem) - expected).abs());
            total += error;
            large += usize::from(error > 0.25);
        }
    }
    (total / pixels as f64, large as f64 / pixels as f64)
}

#[test]
fn reference_decoder_matches_cpu_rasterizer() {
    for (file, characters) in [
        ("InterVariable.ttf", "ag@W8%"),
        ("NotoSansCJK-VF.otf.ttc", "永鬱"),
        ("Font Awesome 7 Free-Solid-900.otf", "\u{f007}\u{f013}"),
        ("NotoSansDevanagari.ttf", "क"),
    ] {
        let data = font_bytes(file);
        let font = Font::new(&data).unwrap();
        for character in characters.chars() {
            let id = glyph(&font, character);
            for ppem in [96.0, 200.0] {
                let (mean, large) = compare(&font, GlyphRequest::new(id, ppem), 0.25);
                assert!(
                    mean < 0.006,
                    "{file} {character} at {ppem}: mean error {mean}"
                );
                assert!(
                    large < 0.0005,
                    "{file} {character} at {ppem}: {large} of pixels off"
                );
            }
        }
    }
}
