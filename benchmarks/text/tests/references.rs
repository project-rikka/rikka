use rikka_test_fonts::find_loaded_font;
use text_parsers::{FONT_CASES, RikkaParser, SkrifaParser, TtfParser, codepoints};
use text_shaping::{BENCHMARK_TEXTS, HarfBuzzEngine, Prepared, RikkaEngine, ShapedGlyph};
use text_unicode::{
    icu_graphemes, icu_nfc, icu_nfd, icu_words, rikka_graphemes, rikka_nfc, rikka_nfd, rikka_words,
    unicode_cases, unicode_rs_graphemes, unicode_rs_nfc, unicode_rs_nfd,
};

const HARFBUZZ_DIVERGENCES: &[&str] = &["hangul"];

fn placed(glyph: &ShapedGlyph) -> (u32, u32, i32, i32, i32, i32) {
    (
        glyph.glyph,
        glyph.cluster,
        glyph.x_advance,
        glyph.y_advance,
        glyph.x_offset,
        glyph.y_offset,
    )
}

fn first_difference(ours: &[ShapedGlyph], theirs: &[ShapedGlyph]) -> Option<String> {
    if ours.len() != theirs.len() {
        return Some(format!("{} glyphs, HarfBuzz {}", ours.len(), theirs.len()));
    }
    ours.iter()
        .zip(theirs)
        .enumerate()
        .find(|(_, (ours, theirs))| placed(ours) != placed(theirs))
        .map(|(index, (ours, theirs))| format!("glyph {index}: {ours:?}, HarfBuzz {theirs:?}"))
}

#[test]
fn shaping_matches_harfbuzz() {
    let mut compared = 0;
    for case in BENCHMARK_TEXTS {
        let Some(data) = find_loaded_font(case.font) else {
            continue;
        };
        let prepared = Prepared::new(&case, case.text, data, false).expect("font should parse");
        let ours = RikkaEngine::new(&prepared)
            .expect("rikka-text-shape should accept the font")
            .shape_corpus_text();
        let theirs = HarfBuzzEngine::new(&prepared)
            .expect("HarfBuzz should accept the font")
            .shape_glyphs(case.text);
        let difference = first_difference(&ours, &theirs);
        match HARFBUZZ_DIVERGENCES.contains(&case.group) {
            true => assert!(
                difference.is_some(),
                "{} now matches HarfBuzz; drop it from the known divergences",
                case.group
            ),
            false => {
                if let Some(difference) = difference {
                    panic!("{} differs from HarfBuzz at {difference}", case.group);
                }
            }
        }
        compared += 1;
    }
    assert!(compared > 0, "no benchmark font is installed");
}

#[test]
fn parsers_agree_on_glyphs_and_advances() {
    let codepoints = codepoints();
    for case in FONT_CASES {
        let Some(data) = find_loaded_font(case.file) else {
            continue;
        };
        let rikka = RikkaParser::open(data, &case).expect("rikka-text-font should read the font");
        let skrifa = SkrifaParser::open(data, &case).expect("skrifa should read the font");
        let ttf = TtfParser::open(data, &case).expect("ttf-parser should read the font");
        let glyphs = rikka.glyphs(&codepoints);
        let ids: Vec<u32> = glyphs.iter().map(|glyph| u32::from(glyph.0)).collect();
        let skrifa_glyphs = skrifa.glyphs(&codepoints);
        let ttf_glyphs = ttf.glyphs(&codepoints);
        assert_eq!(
            ids,
            skrifa_glyphs
                .iter()
                .map(|glyph| glyph.to_u32())
                .collect::<Vec<_>>(),
            "{} charmap differs from skrifa",
            case.name
        );
        assert_eq!(
            ids,
            ttf_glyphs
                .iter()
                .map(|glyph| u32::from(glyph.0))
                .collect::<Vec<_>>(),
            "{} charmap differs from ttf-parser",
            case.name
        );
        let ours = rikka.advances(&glyphs);
        for (engine, theirs) in [
            ("skrifa", skrifa.advances(&skrifa_glyphs)),
            ("ttf-parser", ttf.advances(&ttf_glyphs)),
        ] {
            let worst = ours
                .iter()
                .zip(&theirs)
                .map(|(ours, theirs)| (ours - theirs).abs())
                .fold(0.0f32, f32::max);
            assert!(
                worst <= 1.0,
                "{} advances differ from {engine} by {worst}",
                case.name
            );
        }
    }
}

#[test]
fn segmentation_and_normalization_match_icu4x() {
    for case in unicode_cases() {
        let text = case.text.as_str();
        assert_eq!(
            rikka_graphemes(text),
            icu_graphemes(text),
            "{} graphemes vs ICU4X",
            case.name
        );
        assert_eq!(
            rikka_graphemes(text),
            unicode_rs_graphemes(text),
            "{} graphemes vs unicode-segmentation",
            case.name
        );
        if !text.chars().any(is_ideographic) {
            assert_eq!(
                rikka_words(text),
                icu_words(text),
                "{} words vs ICU4X",
                case.name
            );
        }
        assert_eq!(rikka_nfc(text), icu_nfc(text), "{} NFC vs ICU4X", case.name);
        assert_eq!(rikka_nfd(text), icu_nfd(text), "{} NFD vs ICU4X", case.name);
        assert_eq!(
            rikka_nfc(text),
            unicode_rs_nfc(text),
            "{} NFC vs unicode-normalization",
            case.name
        );
        assert_eq!(
            rikka_nfd(text),
            unicode_rs_nfd(text),
            "{} NFD vs unicode-normalization",
            case.name
        );
    }
}

fn is_ideographic(character: char) -> bool {
    matches!(character, '\u{3040}'..='\u{30FF}' | '\u{3400}'..='\u{9FFF}')
}
