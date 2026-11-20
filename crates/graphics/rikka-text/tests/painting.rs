use std::sync::Arc;

use rikka_test_fonts::{Writer, font_bytes, with_table};
use rikka_text::{
    ColorScheme, GlyphImage, GlyphKind, GlyphRaster, IconFont, IconKey, IconSpec, TextPaint,
    TextSpan, TextSpec,
};
use rikka_text_font::Font;
use rikka_types::Color;

const LIGHT_INK: [u8; 4] = [255, 0, 0, 255];
const DARK_INK: [u8; 4] = [0, 255, 0, 255];

#[test]
fn inks_apply_per_line_byte_ranges() {
    let red = Color::rgb(1.0, 0.0, 0.0);
    let blue = Color::rgb(0.0, 0.0, 1.0);
    let spans = [
        TextSpan::new(8..11).color(blue),
        TextSpan::new(0..3).color(red),
    ];
    let layout = TextSpec::new("one\ntwo six", 14.0).layout(None);
    let paint = TextPaint::new(Arc::clone(&layout)).with_spans(&spans);
    let inks: Vec<Vec<Option<Color>>> = layout
        .rows()
        .map(|row| {
            row.placed((0.0, row.baseline()), 1.0)
                .map(|placed| paint.ink(placed.offset))
                .collect()
        })
        .collect();

    assert!(inks[0].iter().all(|ink| *ink == Some(red)));
    assert_eq!(inks[1][0], None, "second line starts uninked");
    assert_eq!(inks[1].last(), Some(&Some(blue)));
}

fn ink_middle(image: &GlyphImage) -> (f32, f32) {
    let (mut weight, mut x, mut y) = (0.0, 0.0, 0.0);
    for (index, &value) in image.pixels.iter().enumerate() {
        let (row, column) = (index as u32 / image.width, index as u32 % image.width);
        let coverage = f32::from(value);
        weight += coverage;
        x += coverage * (image.left as f32 + column as f32 + 0.5);
        y += coverage * (image.top as f32 - row as f32 - 0.5);
    }
    (x / weight, y / weight)
}

#[test]
fn icons_are_centered_at_any_size() {
    let mut raster = GlyphRaster::new();
    for (glyph, font) in [
        ('\u{e5cd}', IconFont::MaterialSymbols),
        ('-', IconFont::FontAwesomeBrands),
    ] {
        for size in [15.0, 20.0, 24.0, 25.0, 37.5] {
            let spec = IconSpec::new(glyph, size).font(font);
            let image = raster
                .raster_icon(IconKey::from(&spec))
                .expect("icon rasters");
            let (x, y) = ink_middle(&image);
            assert!(
                (x - size / 2.0).abs() < 0.05 && (y - size / 2.0).abs() < 0.05,
                "{font:?} {glyph:?} at {size}px: ink middle ({x}, {y}) is off the box middle {}",
                size / 2.0
            );
        }
    }
}

fn svg_table(glyph: u16, document: &str) -> Vec<u8> {
    let mut table = Writer::new();
    table.u16(0).u32(10).u32(0);
    table
        .u16(1)
        .u16(glyph)
        .u16(glyph)
        .u32(14)
        .u32(document.len() as u32);
    table.raw(document.as_bytes());
    table.finish()
}

fn install_schemed_font(directory: &std::path::Path) {
    let original = font_bytes("JetBrainsMono-Regular.ttf");
    let glyph = Font::new(&original)
        .expect("session font parses")
        .charmap()
        .expect("session font maps characters")
        .map(u32::from('A'))
        .expect("A is mapped")
        .0;
    let document = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg"><style>rect {{ fill: #ff0000 }} @media (prefers-color-scheme: dark) {{ rect {{ fill: #00ff00 }} }}</style><g id="glyph{glyph}"><rect x="0" y="-700" width="600" height="700"/></g></svg>"##
    );
    let font = with_table(&original, b"SVG ", &svg_table(glyph, &document)).expect("font rebuilt");
    std::fs::write(directory.join("schemed.ttf"), font).expect("font written");
    let config = directory.join("fonts.conf");
    std::fs::write(
        &config,
        format!(
            "<fontconfig><dir>{}</dir></fontconfig>",
            directory.display()
        ),
    )
    .expect("configuration written");
    unsafe { std::env::set_var("FONTCONFIG_FILE", &config) };
}

fn middle_pixel(image: &GlyphImage) -> [u8; 4] {
    let at = (((image.height / 2) * image.width + image.width / 2) * 4) as usize;
    image.pixels[at..at + 4]
        .try_into()
        .expect("pixel has four bytes")
}

#[test]
fn svg_glyphs_render_in_the_asked_color_scheme() {
    let directory = tempfile::tempdir().expect("scratch folder");
    install_schemed_font(directory.path());
    let layout = TextSpec::new("A", 40.0).layout(None);
    let row = layout.rows().next().expect("one row");
    let placed = row
        .placed((0.0, row.baseline()), 1.0)
        .next()
        .expect("one glyph");
    assert!(placed.key.is_color(), "the SVG face is a colour face");
    let mut raster = GlyphRaster::new();

    for (scheme, ink) in [
        (ColorScheme::Light, LIGHT_INK),
        (ColorScheme::Dark, DARK_INK),
    ] {
        let image = raster
            .raster(placed.key.with_color_scheme(scheme))
            .expect("glyph rasters");
        assert_eq!(image.kind, GlyphKind::Color);
        assert_eq!(middle_pixel(&image), ink, "{scheme:?}");
    }
}
