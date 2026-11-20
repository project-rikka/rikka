use rikka_text_shape::BufferFlags;

use crate::support::{Request, glyphs, nominal_glyph, shape_text, shape_with};
use rikka_test_fonts::{loaded_font, without_table};

const DOTTED_CIRCLE: u32 = 0x25CC;

#[test]
fn lone_vowel_gets_dotted_circle_unless_disabled() {
    let data = loaded_font("NotoSansDevanagari.ttf");
    let circle = nominal_glyph(data, DOTTED_CIRCLE);
    let circles = |flags| {
        glyphs(&shape_with(
            data,
            "\u{93F}",
            &Request {
                flags,
                ..Request::default()
            },
        ))
        .iter()
        .filter(|&&glyph| glyph == circle)
        .count()
    };
    assert_eq!(circles(BufferFlags::empty()), 1);
    assert_eq!(circles(BufferFlags::BEGINNING_OF_TEXT), 1);
    assert_eq!(circles(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE), 0);
}

#[test]
fn pre_base_parts_precede_base_in_cluster() {
    for (file_name, text, base) in [
        ("NotoSansDevanagari.ttf", "कि", 0x0915),
        ("NotoSansMalayalam.ttf", "കൊ", 0x0D15),
        ("NotoSansKhmer.ttf", "ក្រ", 0x1780),
        ("NotoSansMyanmar.ttf", "ကြ", 0x1000),
        ("NotoSansJavanese.ttf", "ꦏꦺ", 0xA98F),
    ] {
        let data = loaded_font(file_name);
        let shaped = shape_text(data, text);
        let base = nominal_glyph(data, base);
        let drawn = glyphs(&shaped);
        assert_ne!(drawn[0], base, "{file_name}");
        assert!(drawn[1..].contains(&base), "{file_name}");
        assert!(shaped.iter().all(|glyph| glyph.cluster == 0), "{file_name}");
    }
}

#[test]
fn reph_moves_after_base_consonant() {
    let data = loaded_font("NotoSansDevanagari.ttf");
    let shaped = shape_text(data, "र्क");
    assert_eq!(shaped.len(), 2);
    assert_eq!(shaped[0].glyph, nominal_glyph(data, 0x0915));
    assert_ne!(shaped[1].glyph, nominal_glyph(data, 0x0930));
}

#[test]
fn gvar_phantom_points_vary_advances() {
    let data = without_table(loaded_font("InterVariable.ttf"), b"HVAR")
        .expect("Inter has no table directory")
        .leak();
    let advance = |weight: f32| {
        shape_with(
            data,
            "W",
            &Request {
                variations: &[(b"wght", weight)],
                ..Request::default()
            },
        )[0]
        .x_advance
    };
    assert!(advance(900.0) > advance(100.0));
}

#[test]
fn nested_ligature_keeps_skipped_mark() {
    let data = loaded_font("NotoSansJavanese.ttf");
    let shaped = shape_text(data, "ꦨ\u{A9B3}ꦿ");
    assert_eq!(shaped.len(), 2);
    assert_eq!(shaped[1].glyph, nominal_glyph(data, 0xA9B3));
}

#[test]
fn bengali_ra_phala_uses_contextual_blwf() {
    let data = loaded_font("NotoSansBengali.ttf");
    let shaped = shape_text(data, "ন্ত্র");
    assert_eq!(shaped.len(), 2);
    assert!(!glyphs(&shaped).contains(&nominal_glyph(data, 0x09B0)));
}

#[test]
fn left_matra_after_control_is_word_initial() {
    let data = loaded_font("NotoSerifBengali.ttf");
    let after_tab = shape_text(data, "\tকে");
    let after_letter = shape_text(data, "অকে");
    assert_ne!(after_tab[1].glyph, after_letter[1].glyph);
    assert!(!after_tab[1].unsafe_to_break);
    assert!(after_letter[1].unsafe_to_break);
}

#[test]
fn fallback_marks_center_on_cff_base() {
    let data = loaded_font("NotoSerifHentaigana-Regular.otf");
    let shaped = shape_text(data, "\u{1B002}\u{3099}");
    let base_advance = shaped[0].x_advance;
    assert_eq!(shaped[1].x_advance, 0);
    assert!(shaped[1].x_offset < 0 && shaped[1].x_offset > -base_advance);
}
