use rikka_test_fonts::Writer;
use rikka_text_font::{Font, FontFile, GlyphId, VariantGlyph};

#[test]
fn every_mapping_round_trips_through_lookup() {
    for file in [
        "NotoSansCJK-VF.otf.ttc",
        "Font Awesome 7 Brands-Regular-400.otf",
        "NotoNaskhArabicUI.ttf",
        "NotoColorEmoji.ttf",
    ] {
        let data = rikka_test_fonts::font_bytes(file);
        let font = FontFile::new(&data).unwrap().face(0).unwrap();
        let charmap = font.charmap().unwrap();
        let mut previous = None;
        let mut count = 0;
        charmap.for_each_mapping(|codepoint, glyph| {
            assert!(
                previous.is_none_or(|previous| codepoint > previous),
                "{file} {codepoint:#x}"
            );
            assert!(glyph.0 < font.glyph_count() && glyph != GlyphId::NOTDEF);
            assert_eq!(charmap.map(codepoint), Some(glyph), "{file} {codepoint:#x}");
            previous = Some(codepoint);
            count += 1;
        });
        assert!(count > 300, "{file}");
    }
}

#[test]
fn ideographic_variation_sequences_select_variant_glyphs() {
    let data = rikka_test_fonts::font_bytes("NotoSansCJK-VF.otf.ttc");
    let font = FontFile::new(&data).unwrap().face(0).unwrap();
    let charmap = font.charmap().unwrap();
    let base = charmap.map(0x845B).unwrap();
    let Some(VariantGlyph::Glyph(variant)) = charmap.map_variant(0x845B, 0xE0100) else {
        panic!("U+845B U+E0100 has no variant glyph");
    };
    assert_ne!(variant, base);
    assert_eq!(
        charmap.map_variant(0x845B, 0xE0101),
        Some(VariantGlyph::UseDefault)
    );
    assert_eq!(charmap.map_variant(0x41, 0xE0100), None);
}

fn cmap(subtables: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
    let mut table = Writer::new();
    table.u16(0).u16(subtables.len() as u16);
    let mut offset = 4 + subtables.len() * 8;
    for (platform, encoding, data) in subtables {
        table.u16(*platform).u16(*encoding).u32(offset as u32);
        offset += data.len();
    }
    for (_, _, data) in subtables {
        table.raw(data);
    }
    table.finish()
}

fn byte_encoding(glyphs: impl Fn(u8) -> u8) -> Vec<u8> {
    let mut table = Writer::new();
    table.u16(0).u16(262).u16(0);
    for code in 0..=255u8 {
        table.u8(glyphs(code));
    }
    table.finish()
}

fn segment_mapping(segments: &[(u16, u16, i16)]) -> Vec<u8> {
    let mut all = segments.to_vec();
    all.push((0xFFFF, 0xFFFF, 1));
    let count = all.len() as u16;
    let mut table = Writer::new();
    table
        .u16(4)
        .u16(0)
        .u16(0)
        .u16(count * 2)
        .u16(0)
        .u16(0)
        .u16(0);
    for &(_, end, _) in &all {
        table.u16(end);
    }
    table.u16(0);
    for &(start, _, _) in &all {
        table.u16(start);
    }
    for &(_, _, delta) in &all {
        table.i16(delta);
    }
    for _ in &all {
        table.u16(0);
    }
    table.finish()
}

fn font_with_cmap(table: Vec<u8>) -> Vec<u8> {
    crate::support::sfnt(100, &[(b"cmap", table)])
}

#[test]
fn mac_roman_subtable_maps_unicode() {
    let data = font_with_cmap(cmap(&[(
        1,
        0,
        byte_encoding(|code| {
            if code == 0x8E {
                7
            } else if code == b'A' {
                3
            } else {
                0
            }
        }),
    )]));
    let charmap = Font::new(&data).unwrap().charmap().unwrap();
    assert_eq!(charmap.map(u32::from('é')), Some(GlyphId(7)));
    assert_eq!(charmap.map(u32::from('A')), Some(GlyphId(3)));
    assert_eq!(charmap.map(0x8E), None);
    let mut mappings = Vec::new();
    charmap.for_each_mapping(|codepoint, glyph| mappings.push((codepoint, glyph.0)));
    assert_eq!(mappings, [(u32::from('A'), 3), (u32::from('é'), 7)]);
}

#[test]
fn full_repertoire_subtable_preferred_over_bmp() {
    let mut groups = Writer::new();
    groups.u16(12).u16(0).u32(28 + 12).u32(0).u32(2);
    groups.u32(0x41).u32(0x43).u32(10);
    groups.u32(0x1F600).u32(0x1F600).u32(20);
    let mut constant = Writer::new();
    constant
        .u16(13)
        .u16(0)
        .u32(28)
        .u32(0)
        .u32(1)
        .u32(0x20000)
        .u32(0x2FFFF)
        .u32(9);
    let data = font_with_cmap(cmap(&[
        (3, 1, segment_mapping(&[(0x41, 0x5A, 50)])),
        (3, 10, groups.finish()),
    ]));
    let charmap = Font::new(&data).unwrap().charmap().unwrap();
    assert_eq!(charmap.subtable().format, 12);
    assert_eq!(charmap.map(0x42), Some(GlyphId(11)));
    assert_eq!(charmap.map(0x1F600), Some(GlyphId(20)));
    assert_eq!(charmap.map(0x44), None);
    let data = font_with_cmap(cmap(&[(3, 10, constant.finish())]));
    let charmap = Font::new(&data).unwrap().charmap().unwrap();
    assert_eq!(charmap.map(0x2ABCD), Some(GlyphId(9)));
    assert_eq!(charmap.map(0x30000), None);
}
