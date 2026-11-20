use rikka_text_font::FontFile;
use rikka_text_shape::{BufferFlags, Direction};
use rikka_text_unicode::Script;

use crate::support::{Request, Shaped, SyntheticFont, glyphs, shape_text, shape_with};
use rikka_test_fonts::loaded_font;

fn rtl(script: Script) -> Request<'static> {
    Request {
        script: Some(script),
        direction: Some(Direction::RightToLeft),
        ..Request::default()
    }
}

fn clusters(shaped: &[Shaped]) -> Vec<u32> {
    shaped.iter().map(|glyph| glyph.cluster).collect()
}

#[test]
fn clusters_are_offsets_from_base() {
    let data = loaded_font("NotoSans.ttf");
    let file = FontFile::new(data).unwrap();
    let font = file.face(0).unwrap();
    let face = rikka_text_shape::ShapeFace::new(font, rikka_text_font::Location::default());
    let mut buffer = rikka_text_shape::Buffer::new();
    buffer.push_str("héllo", 100);
    let properties = buffer.guess_properties();
    face.plan(&properties, &[]).shape(&mut buffer);
    let clusters: Vec<u32> = buffer
        .glyph_infos()
        .iter()
        .map(|info| info.cluster())
        .collect();
    assert_eq!(clusters, [100, 101, 103, 104, 105]);
}

#[test]
fn standard_ligatures_join_letters_unless_disabled() {
    let data = loaded_font("NotoSans.ttf");
    let ligated = shape_text(data, "fi");
    let separate = shape_with(
        data,
        "fi",
        &Request {
            features: "liga=0",
            ..Request::default()
        },
    );
    assert_eq!(ligated.len(), 1);
    assert_eq!(ligated[0].cluster, 0);
    assert_eq!(separate.len(), 2);
}

#[test]
fn arabic_letters_take_joining_forms() {
    let data = loaded_font("NotoSansArabic.ttf");
    let isolated = glyphs(&shape_with(data, "ب", &rtl(Script::Arabic)));
    let separated = glyphs(&shape_with(data, "ب ب ب", &rtl(Script::Arabic)));
    let joined = glyphs(&shape_with(data, "ببب", &rtl(Script::Arabic)));
    let space = glyphs(&shape_with(data, " ", &rtl(Script::Arabic)));
    let isolated_letters: Vec<u16> = separated
        .into_iter()
        .filter(|glyph| !space.contains(glyph))
        .collect();
    assert_eq!(
        isolated_letters,
        [isolated.clone(), isolated.clone(), isolated].concat()
    );
    assert_ne!(joined, isolated_letters);
}

#[test]
fn arabic_fallback_uses_presentation_forms() {
    let data = SyntheticFont::new(16)
        .map(0x0628, 1)
        .map(0xFE8F, 2)
        .map(0xFE90, 3)
        .map(0xFE91, 4)
        .map(0xFE92, 5)
        .map(0x0644, 6)
        .map(0x0627, 7)
        .map(0xFEDF, 8)
        .map(0xFEE0, 9)
        .map(0xFEDD, 10)
        .map(0xFEDE, 11)
        .map(0xFE8D, 12)
        .map(0xFE8E, 13)
        .map(0xFEFB, 14)
        .map(0xFEFC, 15)
        .build();
    assert_eq!(
        glyphs(&shape_with(data, "ببب", &rtl(Script::Arabic))),
        [3, 5, 4]
    );
    assert_eq!(glyphs(&shape_with(data, "ب", &rtl(Script::Arabic))), [2]);
    let lam_alef = shape_with(data, "لا", &rtl(Script::Arabic));
    assert_eq!(glyphs(&lam_alef), [14]);
    assert_eq!(clusters(&lam_alef), [0]);
}

#[test]
fn default_ignorables_are_hidden_or_removed() {
    let data = loaded_font("NotoSans.ttf");
    let space = shape_text(data, " ")[0].glyph;
    let hidden = shape_text(data, "a\u{200B}b");
    assert_eq!(hidden.len(), 3);
    assert_eq!(hidden[1].glyph, space);
    assert_eq!(hidden[1].x_advance, 0);
    let removed = shape_with(
        data,
        "a\u{200B}b",
        &Request {
            flags: BufferFlags::REMOVE_DEFAULT_IGNORABLES,
            ..Request::default()
        },
    );
    assert_eq!(removed.len(), 2);
}

#[test]
fn kern_table_applies_without_gpos() {
    let data = SyntheticFont::new(3)
        .map(0x41, 1)
        .map(0x56, 2)
        .kern(1, 2, -300)
        .build();
    let shaped = shape_text(data, "AV");
    assert_eq!(shaped[0].x_advance + shaped[1].x_advance, 700);
    assert_eq!(shaped[1].x_offset, -150);
}

#[test]
fn rtl_runs_mirror_brackets() {
    let data = loaded_font("NotoSansHebrew.ttf");
    let closing = shape_with(
        data,
        ")",
        &Request {
            script: Some(Script::Latin),
            ..Request::default()
        },
    )[0]
    .glyph;
    let mirrored = shape_with(data, "(", &rtl(Script::Hebrew))[0].glyph;
    assert_eq!(mirrored, closing);
}

#[test]
fn joined_arabic_is_unsafe_to_break() {
    let data = loaded_font("NotoSansArabic.ttf");
    let shaped = shape_with(data, "بب", &rtl(Script::Arabic));
    assert!(shaped.iter().any(|glyph| glyph.unsafe_to_break));
}
