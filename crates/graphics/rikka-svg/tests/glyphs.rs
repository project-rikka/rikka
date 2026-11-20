use rikka_svg::{GlyphPixels, Raster, Svg};
use rikka_test_fonts::{Writer, font_bytes, sfnt};
use rikka_text_font::Font;

const HEADER: usize = 12;
const RECORD: usize = 16;
const GREEN: [u8; 4] = [0, 255, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

const GLYPHS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg">
    <g id="glyph3"><rect x="0" y="-800" width="500" height="800" fill="#ff0000"/></g>
    <g id="glyph4" transform="translate(100 0)"><rect x="0" y="-400" width="200" height="400" fill="#0000ff" fill-opacity="0.5"/></g>
</svg>"##;

fn pixel(glyph: &GlyphPixels, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * glyph.width + x) * 4) as usize;
    glyph.premultiplied[at..at + 4]
        .try_into()
        .expect("pixel has four bytes")
}

#[test]
fn renders_single_glyph_above_baseline() {
    let svg = Svg::from_bytes(GLYPHS.as_bytes()).expect("parses");

    let glyph = svg
        .render_glyph(3, 0.02, 0.0)
        .expect("renders")
        .expect("glyph has ink");

    assert_eq!((glyph.left, glyph.top), (0, 16));
    assert_eq!((glyph.width, glyph.height), (10, 16));
    assert_eq!(pixel(&glyph, 5, 8), [255, 0, 0, 255]);
}

#[test]
fn translucent_glyph_is_premultiplied_and_transformed() {
    let svg = Svg::from_bytes(GLYPHS.as_bytes()).expect("parses");

    let glyph = svg
        .render_glyph(4, 0.02, 0.0)
        .expect("renders")
        .expect("glyph has ink");

    assert_eq!((glyph.left, glyph.top), (2, 8));
    let [red, green, blue, alpha] = pixel(&glyph, 2, 4);
    assert_eq!((red, green), (0, 0));
    assert!(alpha.abs_diff(128) <= 1 && blue.abs_diff(alpha) <= 1);
}

fn tables(font: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let count = usize::from(u16::from_be_bytes([font[4], font[5]]));
    (0..count)
        .map(|index| {
            let record = &font[HEADER + index * RECORD..];
            let word = |at: usize| u32::from_be_bytes(record[at..at + 4].try_into().unwrap());
            let (offset, length) = (word(8) as usize, word(12) as usize);
            (
                record[..4].try_into().unwrap(),
                font[offset..offset + length].to_vec(),
            )
        })
        .collect()
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

fn colr_tables(glyph: u16) -> (Vec<u8>, Vec<u8>) {
    let mut colr = Writer::new();
    colr.u16(0).u16(1).u32(14).u32(20).u16(1);
    colr.u16(glyph).u16(0).u16(1);
    colr.u16(glyph).u16(0);
    let mut cpal = Writer::new();
    cpal.u16(0).u16(1).u16(1).u16(1).u32(14).u16(0);
    cpal.raw(&[0, 0, 255, 255]);
    (colr.finish(), cpal.finish())
}

fn colored_font(directory: &std::path::Path) {
    let original = font_bytes("JetBrainsMono-Regular.ttf");
    let parsed = Font::new(&original).expect("session font parses");
    let charmap = parsed.charmap().expect("session font maps characters");
    let [svg, colr] = ['A', 'B'].map(|character| {
        charmap
            .map(u32::from(character))
            .expect("character is mapped")
            .0
    });
    let document = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg"><g id="glyph{svg}"><rect x="0" y="-700" width="600" height="700" fill="#00ff00"/><text y="-100" font-size="400">A</text></g></svg>"##
    );
    let (colr_table, cpal_table) = colr_tables(colr);
    let mut tables = tables(&original);
    tables.push((*b"SVG ", svg_table(svg, &document)));
    tables.push((*b"COLR", colr_table));
    tables.push((*b"CPAL", cpal_table));
    let named: Vec<(&[u8; 4], &[u8])> = tables
        .iter()
        .map(|(tag, data)| (tag, data.as_slice()))
        .collect();
    std::fs::write(directory.join("colored.ttf"), sfnt(&named)).expect("font written");
}

fn raster_pixel(raster: &Raster, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * raster.width() + x) * 4) as usize;
    raster.pixels()[at..at + 4]
        .try_into()
        .expect("pixel has four bytes")
}

fn cell_has(raster: &Raster, columns: std::ops::Range<u32>, wanted: [u8; 4]) -> bool {
    columns.into_iter().any(|x| {
        (0..raster.height()).any(|y| {
            let found = raster_pixel(raster, x, y);
            found[3] == 255
                && found
                    .iter()
                    .zip(wanted)
                    .all(|(got, want)| got.abs_diff(want) <= 8)
        })
    })
}

#[test]
fn color_glyphs_draw_through_svg_colr_and_outline_paths() {
    let directory = tempfile::tempdir().expect("scratch folder");
    colored_font(directory.path());
    let config = directory.path().join("fonts.conf");
    std::fs::write(
        &config,
        format!(
            "<fontconfig><dir>{}</dir></fontconfig>",
            directory.path().display()
        ),
    )
    .expect("configuration written");
    unsafe { std::env::set_var("FONTCONFIG_FILE", &config) };
    let document = r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="48">
        <text x="0" y="40" font-family="JetBrains Mono" font-size="40" fill="#0000ff">ABC</text>
    </svg>"##;
    let raster = Svg::from_bytes(document.as_bytes())
        .expect("parses")
        .render(72, 48)
        .expect("renders");

    assert_eq!(raster_pixel(&raster, 12, 30), GREEN, "SVG glyph document");
    assert!(
        cell_has(&raster, 24..48, RED),
        "COLR layers in their palette colour"
    );
    assert!(
        !cell_has(&raster, 24..48, BLUE),
        "COLR glyph ignores the fill"
    );
    assert!(
        cell_has(&raster, 48..72, BLUE),
        "plain glyph of a colour font is filled"
    );
}
