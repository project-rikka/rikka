use rikka_text_document::{Content, Endpoints, Granularity, SelectionOrigin};
use rikka_text_layout::{Affinity, WordRule};

#[test]
fn line_units_cover_their_break() {
    for (text, at, line) in [
        ("first\nsecond\nlast", 2, "first\n"),
        ("first\nsecond\nlast", 8, "second\n"),
        ("first\nsecond\nlast", 15, "last"),
        ("only", 1, "only"),
        ("ends\n", 5, ""),
    ] {
        let content = Content::new(text);
        let unit = content.unit_at(at, Affinity::After, Granularity::Line, WordRule::Prose);
        assert_eq!(&text[unit], line, "{text:?} at {at}");
    }
}

#[test]
fn extending_keeps_the_origin_unit() {
    let word = SelectionOrigin::new(Granularity::Word, 4..9);
    let caret = SelectionOrigin::new(Granularity::Character, 6..6);
    for (origin, unit, anchor, focus) in [
        (&word, 4..9, 4, 9),
        (&word, 5..5, 4, 9),
        (&word, 10..15, 4, 15),
        (&word, 9..10, 4, 10),
        (&word, 0..3, 9, 0),
        (&word, 3..4, 9, 3),
        (&caret, 6..6, 6, 6),
        (&caret, 2..2, 6, 2),
        (&caret, 11..11, 6, 11),
    ] {
        assert_eq!(
            origin.extend(unit.clone()),
            Endpoints { anchor, focus },
            "{origin:?} to {unit:?}"
        );
    }
}
