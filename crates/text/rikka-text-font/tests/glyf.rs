use crate::support::Path;
use rikka_text_font::{
    Bounds, Font, FontFile, GlyphId, Location, OutlineFormat, OutlineScratch, Tag,
};

fn glyph(font: &Font<'_>, character: char) -> GlyphId {
    font.charmap().unwrap().map(u32::from(character)).unwrap()
}

fn draw(font: &Font<'_>, glyph: GlyphId, location: &Location) -> (Path, Bounds) {
    let mut path = Path::default();
    font.outline(glyph, location, &mut OutlineScratch::new(), &mut path)
        .unwrap();
    let bounds = path.bounds();
    (path, bounds)
}

#[test]
fn default_outlines_match_glyf_bounds() {
    let mut scratch = OutlineScratch::new();
    for file in [
        "InterVariable.ttf",
        "JetBrainsMono-Regular.ttf",
        "NotoSansDevanagari.ttf",
        "NotoSansArabic.ttf",
    ] {
        let data = rikka_test_fonts::font_bytes(file);
        let font = Font::new(&data).unwrap();
        assert_eq!(font.outline_format(), Some(OutlineFormat::TrueType));
        for id in 0..font.glyph_count() {
            let mut path = Path::default();
            font.outline(GlyphId(id), &Location::default(), &mut scratch, &mut path)
                .unwrap();
            let drawn = path.bounds();
            let header = font
                .glyph_bounds(GlyphId(id), &Location::default(), &mut scratch)
                .unwrap();
            assert_eq!(drawn, header, "{file} glyph {id}");
        }
    }
}

#[test]
fn composite_glyphs_place_components() {
    let data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let (base, base_bounds) = draw(&font, glyph(&font, 'A'), &Location::default());
    let (accented, accented_bounds) = draw(&font, glyph(&font, 'Á'), &Location::default());
    assert!(accented.contour_count() > base.contour_count());
    assert_eq!(accented_bounds.y_min, base_bounds.y_min);
    assert!(accented_bounds.y_max > base_bounds.y_max + 200.0);
    assert!(
        accented_bounds.x_min <= base_bounds.x_min && accented_bounds.x_max >= base_bounds.x_max
    );
    let base_segments = &base.segments[..];
    assert!(
        accented
            .segments
            .windows(base_segments.len())
            .any(|window| window == base_segments)
    );
}

#[test]
fn variable_outlines_follow_weight_axis() {
    let data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let wght = Tag::new(b"wght");
    let l = glyph(&font, 'l');
    let (_, thin) = draw(&font, l, &font.location([(wght, 100.0)]));
    let (_, regular) = draw(&font, l, &font.location([(wght, 400.0)]));
    let (_, black) = draw(&font, l, &font.location([(wght, 900.0)]));
    assert!(thin.width() < regular.width() && regular.width() < black.width());
    assert!(font.location([(wght, 400.0)]).is_default());
    assert_eq!(draw(&font, l, &Location::default()).1, regular);
}

#[test]
fn static_instances_match_variable_font() {
    let variable_data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let variable = Font::new(&variable_data).unwrap();
    let collection_data = rikka_test_fonts::font_bytes("Inter.ttc");
    let collection = FontFile::new(&collection_data).unwrap();
    let mut scratch = OutlineScratch::new();
    let mut compared = 0;
    for face in collection.faces() {
        let face = face.unwrap();
        let name = face.name().unwrap();
        let family = name.get(rikka_text_font::NameId::FAMILY).unwrap();
        let subfamily = name.get(rikka_text_font::NameId::SUBFAMILY).unwrap();
        if family.contains("Display") || subfamily.contains("Italic") {
            continue;
        }
        let weight = f32::from(face.os2().unwrap().weight_class());
        let location = variable.location([(Tag::new(b"wght"), weight), (Tag::new(b"opsz"), 14.0)]);
        let metrics = variable.glyph_metrics(&location);
        let default = Location::default();
        let static_metrics = face.glyph_metrics(&default);
        let charmap = variable.charmap().unwrap();
        let static_charmap = face.charmap().unwrap();
        for character in ('!'..='~').chain('À'..='ÿ') {
            let variable_glyph = charmap.map(u32::from(character)).unwrap();
            let static_glyph = static_charmap.map(u32::from(character)).unwrap();
            let advance = metrics.advance_width(variable_glyph).unwrap();
            let static_advance = static_metrics.advance_width(static_glyph).unwrap();
            assert!(
                (advance - static_advance).abs() <= 1.0,
                "{family} {character}: {advance} vs {static_advance}"
            );
            let varied = variable
                .glyph_bounds(variable_glyph, &location, &mut scratch)
                .unwrap();
            let fixed = face
                .glyph_bounds(static_glyph, &Location::default(), &mut scratch)
                .unwrap();
            for (a, b) in [
                (varied.x_min, fixed.x_min),
                (varied.y_min, fixed.y_min),
                (varied.x_max, fixed.x_max),
                (varied.y_max, fixed.y_max),
            ] {
                assert!(
                    (a - b).abs() <= 4.0,
                    "{family} {character}: {varied:?} vs {fixed:?}"
                );
            }
            compared += 1;
        }
    }
    assert!(compared > 1000);
}
