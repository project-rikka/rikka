use rikka_text_catalog::{FaceMatch, FallbackRequest, Locale};
use rikka_text_unicode::Script;

use crate::support::{family, matched, query, session};

fn inter() -> FaceMatch {
    matched(&query(&["Inter"]))
}

fn fallback_family(primary: FaceMatch, cluster: &str) -> &'static str {
    family(
        session()
            .fallback(&FallbackRequest::new(primary, cluster))
            .face(),
    )
}

fn in_locale(cluster: &str, locale: &str) -> &'static str {
    let request = FallbackRequest::new(inter(), cluster)
        .script(Script::Han)
        .locale(Locale::parse(locale).expect("invalid locale"));
    family(session().fallback(&request).face())
}

#[test]
fn covered_clusters_keep_primary_face() {
    let primary = inter();
    for cluster in [
        "a",
        "é",
        "e\u{301}",
        "Ж",
        "©",
        "1",
        "\u{200D}",
        "\n",
        "a\u{FE0F}",
    ] {
        let found = session().fallback(&FallbackRequest::new(primary, cluster));
        assert_eq!(found, primary, "{cluster:?}");
    }
}

#[test]
fn complex_scripts_fall_back_to_noto() {
    let primary = inter();
    for (cluster, expected) in [
        ("ع", "Noto Sans Arabic"),
        ("لا", "Noto Sans Arabic"),
        ("क्ष", "Noto Sans Devanagari"),
        ("กำ", "Noto Sans Thai"),
        ("ᱚ", "Noto Sans Ol Chiki"),
        ("ⵜ", "Noto Sans Tifinagh"),
        ("Ꭰ", "Noto Sans Cherokee"),
        ("ሀ", "Noto Sans Ethiopic"),
        ("א", "Noto Sans Hebrew"),
    ] {
        assert_eq!(fallback_family(primary, cluster), expected, "{cluster}");
    }
}

#[test]
fn han_region_follows_locale() {
    assert_eq!(in_locale("直", "ja"), "Noto Sans CJK JP");
    assert_eq!(in_locale("直", "ja_JP.UTF-8"), "Noto Sans CJK JP");
    assert_eq!(in_locale("直", "ko-KR"), "Noto Sans CJK KR");
    assert_eq!(in_locale("直", "zh-Hans"), "Noto Sans CJK SC");
    assert_eq!(in_locale("直", "zh_CN"), "Noto Sans CJK SC");
    assert_eq!(in_locale("直", "zh-Hant"), "Noto Sans CJK TC");
    assert_eq!(in_locale("直", "zh-TW"), "Noto Sans CJK TC");
    assert_eq!(in_locale("直", "zh-HK"), "Noto Sans CJK HK");
    let unlocalized = FallbackRequest::new(inter(), "直");
    assert_eq!(
        family(session().fallback(&unlocalized).face()),
        "Noto Sans CJK SC"
    );
}

#[test]
fn emoji_presentation_prefers_color_faces() {
    let primary = inter();
    for cluster in [
        "😀",
        "☺\u{FE0F}",
        "👨\u{200D}👩\u{200D}👧",
        "👍🏽",
        "🇯🇵",
        "1\u{FE0F}\u{20E3}",
        "🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
    ] {
        let found = session().fallback(&FallbackRequest::new(primary, cluster));
        let face = session().face(found.face());
        assert!(
            face.is_color(),
            "{cluster:?} fell back to {}",
            face.family()
        );
        assert_eq!(face.family(), "Noto Color Emoji");
    }
}

#[test]
fn uncovered_codepoints_keep_primary_face() {
    let primary = inter();
    for cluster in ["\u{10FFFD}", "\u{E0FFF}", "a\u{10FFFD}"] {
        assert_eq!(
            session().fallback(&FallbackRequest::new(primary, cluster)),
            primary
        );
    }
}
