use rikka_text_layout::{Caps, Paragraph, TextStyle, Weight, Wrap};

use crate::support::{assert_close, family, layout_text, params, shape, wrapped};

#[test]
fn caps_select_the_font_features_through_line_breaks() {
    let text = "small capitals Mix";
    let ids = |caps: Caps, width: Option<f32>| -> Vec<u16> {
        let shaped = Paragraph::new(text, &TextStyle::new(["Noto Sans"]).caps(caps)).shape();
        let layout = match width {
            Some(width) => shaped.layout(&wrapped(width, Wrap::Word)),
            None => shaped.layout(&params()),
        };
        layout.all_glyphs().iter().map(|glyph| glyph.id).collect()
    };
    let normal = ids(Caps::Normal, None);
    let small = ids(Caps::Small, None);
    let all_small = ids(Caps::AllSmall, None);
    assert_ne!(normal[0], small[0], "lowercase becomes a small capital");
    assert_eq!(normal[15], small[15], "capitals stay with small-caps");
    assert_ne!(
        small[15], all_small[15],
        "capitals shrink with all-small-caps"
    );
    assert_eq!(small[1..14], all_small[1..14]);
    assert_eq!(
        ids(Caps::Small, Some(60.0)),
        small,
        "wrapped rows keep them"
    );
}

#[test]
fn variable_face_keeps_distinct_weights() {
    let location = |weight: f32| {
        Paragraph::new(
            "H",
            &TextStyle::new(["Inter Variable"]).weight(Weight(weight)),
        )
        .shape()
        .layout(&params())
        .all_glyphs()[0]
            .location
    };
    assert!(!location(450.25).is_default());
    assert_ne!(location(450.0), location(450.25));
    assert_eq!(location(450.0), location(450.0001));
}

#[test]
fn fallback_switches_faces_mid_word() {
    let text = "Helloनमस्तेこんにちは";
    let layout = layout_text(text, &params());
    let glyphs = layout.all_glyphs();
    let family_at = |byte: usize| {
        let glyph = glyphs
            .iter()
            .find(|glyph| glyph.cluster().contains(&byte))
            .unwrap_or_else(|| panic!("no glyph covers byte {byte}"));
        family(glyph.face)
    };
    assert!(family_at(0).starts_with("Inter"), "{}", family_at(0));
    assert_eq!(family_at("Hello".len()), "Noto Sans Devanagari");
    let japanese = text.find('こ').unwrap();
    assert!(
        family_at(japanese).contains("CJK"),
        "{}",
        family_at(japanese)
    );
    assert!(
        glyphs.iter().all(|glyph| glyph.id != 0),
        "nothing is missing"
    );
}

#[test]
fn arabic_rows_match_reshaped_rows() {
    let text = "الاستقلالية والمسؤولية";
    let shaped = shape(text);
    for width in [40.0, 55.0, 70.0] {
        let layout = shaped.layout(&wrapped(width, Wrap::Glyph));
        assert!(layout.rows().len() > 1);
        let mid_word = layout.rows().iter().any(|row| {
            row.text_range().end < text.len() && !layout.text()[row.text_range()].ends_with(' ')
        });
        assert!(mid_word, "a break falls inside a word at width {width}");
        for row in layout.rows() {
            let row_text = layout.text()[row.text_range()].trim_end();
            let alone = layout_text(row_text, &params());
            let ids = |glyphs: &[rikka_text_layout::Glyph]| -> Vec<u16> {
                glyphs.iter().map(|glyph| glyph.id).collect()
            };
            let row_glyphs: Vec<_> = layout
                .glyphs(row)
                .iter()
                .filter(|glyph| (glyph.start as usize) < row.text_range().start + row_text.len())
                .copied()
                .collect();
            assert_eq!(
                ids(&row_glyphs),
                ids(alone.all_glyphs()),
                "row {row_text:?} at width {width}"
            );
        }
    }
}

#[test]
fn fallback_faces_keep_baseline() {
    let baseline = layout_text("H", &params()).rows()[0].baseline();
    for text in ["あ", "日本語", "Latin 日本語", "😀", "नमस्ते", ""] {
        assert_close(
            layout_text(text, &params()).rows()[0].baseline(),
            baseline,
            &format!("baseline of {text:?}"),
        );
    }
}
