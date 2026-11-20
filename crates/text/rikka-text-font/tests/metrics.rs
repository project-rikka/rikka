use rikka_test_fonts::Writer;
use rikka_text_font::{Font, FontFile, Location, NameId, Tag};

#[test]
fn variable_metrics_match_static_instances() {
    let variable_data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let variable = Font::new(&variable_data).unwrap();
    let collection_data = rikka_test_fonts::font_bytes("Inter.ttc");
    let mut compared = 0;
    for face in FontFile::new(&collection_data).unwrap().faces() {
        let face = face.unwrap();
        let name = face.name().unwrap();
        if name.get(NameId::FAMILY).unwrap().contains("Display")
            || name.get(NameId::SUBFAMILY).unwrap().contains("Italic")
        {
            continue;
        }
        let weight = f32::from(face.os2().unwrap().weight_class());
        let location = variable.location([(Tag::new(b"wght"), weight)]);
        let varied = variable.metrics(&location);
        let fixed = face.metrics(&Location::default());
        for (what, a, b) in [
            ("ascender", varied.ascender, fixed.ascender),
            ("descender", varied.descender, fixed.descender),
            ("line gap", varied.line_gap, fixed.line_gap),
            (
                "underline position",
                varied.underline_position,
                fixed.underline_position,
            ),
            (
                "underline thickness",
                varied.underline_thickness,
                fixed.underline_thickness,
            ),
            (
                "strikeout position",
                varied.strikeout_position,
                fixed.strikeout_position,
            ),
            (
                "strikeout thickness",
                varied.strikeout_thickness,
                fixed.strikeout_thickness,
            ),
            (
                "x-height",
                varied.x_height.unwrap(),
                fixed.x_height.unwrap(),
            ),
            (
                "cap height",
                varied.cap_height.unwrap(),
                fixed.cap_height.unwrap(),
            ),
        ] {
            assert!((a - b).abs() <= 1.0, "{weight} {what}: {a} vs {b}");
        }
        compared += 1;
    }
    assert_eq!(compared, 9);
    let thin = variable.metrics(&variable.location([(Tag::new(b"wght"), 100.0)]));
    let black = variable.metrics(&variable.location([(Tag::new(b"wght"), 900.0)]));
    assert!(thin.underline_thickness < black.underline_thickness);
}

fn os2_table(selection: u16) -> Vec<u8> {
    let mut table = Writer::new();
    table.u16(4).i16(500).u16(400).u16(5).u16(0);
    for _ in 0..8 {
        table.i16(0);
    }
    table
        .i16(50)
        .i16(300)
        .i16(0)
        .raw(&[0; 10])
        .raw(&[0; 16])
        .raw(b"TEST")
        .u16(selection)
        .u16(0x20)
        .u16(0x7E);
    table
        .i16(800)
        .i16(-200)
        .i16(50)
        .u16(1100)
        .u16(400)
        .u32(1)
        .u32(0)
        .i16(480)
        .i16(700)
        .u16(0)
        .u16(0x20)
        .u16(2);
    table.finish()
}

fn hhea_table() -> Vec<u8> {
    let mut table = Writer::new();
    table.u32(0x0001_0000).i16(900).i16(-300).i16(20);
    for _ in 0..12 {
        table.u16(0);
    }
    table.u16(1);
    table.finish()
}

#[test]
fn line_metrics_honor_use_typo_metrics() {
    let typo = crate::support::sfnt(1, &[(b"OS/2", os2_table(1 << 7)), (b"hhea", hhea_table())]);
    let metrics = Font::new(&typo).unwrap().metrics(&Location::default());
    assert_eq!(
        (metrics.ascender, metrics.descender, metrics.line_gap),
        (800.0, -200.0, 50.0)
    );
    assert_eq!(
        (metrics.x_height, metrics.cap_height),
        (Some(480.0), Some(700.0))
    );
    assert_eq!(
        (metrics.strikeout_position, metrics.strikeout_thickness),
        (300.0, 50.0)
    );
    let horizontal = crate::support::sfnt(1, &[(b"OS/2", os2_table(0)), (b"hhea", hhea_table())]);
    let metrics = Font::new(&horizontal)
        .unwrap()
        .metrics(&Location::default());
    assert_eq!(
        (metrics.ascender, metrics.descender, metrics.line_gap),
        (900.0, -300.0, 20.0)
    );
    let without_hhea = crate::support::sfnt(1, &[(b"OS/2", os2_table(0))]);
    let metrics = Font::new(&without_hhea)
        .unwrap()
        .metrics(&Location::default());
    assert_eq!((metrics.ascender, metrics.descender), (800.0, -200.0));
}
