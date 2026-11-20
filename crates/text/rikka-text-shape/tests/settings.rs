use rikka_text_font::Tag;
use rikka_text_shape::Feature;
use std::str::FromStr;

#[test]
fn parses_feature_values_and_ranges() {
    let cases = [
        ("liga", Feature::new(Tag::new(b"liga"), 1)),
        ("+dlig", Feature::new(Tag::new(b"dlig"), 1)),
        ("-calt", Feature::new(Tag::new(b"calt"), 0)),
        ("liga=0", Feature::new(Tag::new(b"liga"), 0)),
        ("aalt=2", Feature::new(Tag::new(b"aalt"), 2)),
        ("ss01 = on", Feature::new(Tag::new(b"ss01"), 1)),
        ("'kern'=off", Feature::new(Tag::new(b"kern"), 0)),
        (
            "kern[3:5]=0",
            Feature::new(Tag::new(b"kern"), 0).with_clusters(3..5),
        ),
        (
            "smcp[2]",
            Feature::new(Tag::new(b"smcp"), 1).with_clusters(2..3),
        ),
        (
            "tnum[4:]",
            Feature::new(Tag::new(b"tnum"), 1).with_clusters(4..u32::MAX),
        ),
        ("cv01", Feature::new(Tag::new(b"cv01"), 1)),
    ];
    for (text, expected) in cases {
        assert_eq!(Feature::from_str(text), Ok(expected), "{text}");
    }
}

#[test]
fn malformed_feature_settings_are_rejected() {
    for text in ["", "liga=", "toolong", "'liga", "liga[2", "=1", "liga x"] {
        assert!(Feature::from_str(text).is_err(), "{text}");
    }
}
