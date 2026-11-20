use rikka_ime::{Preedit, Property, Segment, SegmentStyle};

#[test]
fn property_parses_fields_and_hints() {
    let current =
        Property::parse("/Fcitx/im:Mozc:mozc:Hiragana:menu,label=あ:B").expect("property");
    assert_eq!(current.key, "/Fcitx/im");
    assert_eq!(current.label, "Mozc");
    assert_eq!(current.icon, "mozc");
    assert_eq!(current.tip, "Hiragana");
    assert_eq!(current.hint, "menu,label=あ:B");
    assert_eq!(current.badge(), Some("あ:B"));
    assert!(current.opens_menu());
    assert!(current.is_current_input_method());

    let action = Property::parse("/Fcitx/fullwidth:Half:i:Half width:").expect("property");
    assert_eq!(action.badge(), None);
    assert!(!action.opens_menu());

    let entry = Property::parse("/Fcitx/im/mozc:Mozc:mozc::").expect("property");
    assert_eq!(entry.input_method(), Some("mozc"));
    assert!(!entry.is_current_input_method());

    assert!(Property::parse("/Fcitx/im:a:b").is_none());
    assert!(Property::parse("").is_none());
}

#[test]
fn preedit_parses_runs_and_byte_caret() {
    assert_eq!(
        Preedit::marked("きょうは", "", 4).segments(),
        [Segment::new("きょうは", SegmentStyle::Underline)]
    );
    assert_eq!(
        Preedit::marked("きょうはいい", "3:3:3:255", 0).segments(),
        [
            Segment::new("きょう", SegmentStyle::Underline),
            Segment::new("はいい", SegmentStyle::Highlight),
        ]
    );
    assert_eq!(
        Preedit::marked("あい", "3:1:99:0", 0).segments(),
        [
            Segment::new("あ", SegmentStyle::Underline),
            Segment::new("い", SegmentStyle::Highlight),
        ],
        "run past end marked beyond text"
    );
    assert_eq!(Preedit::marked("きょう", "", 2).cursor(), 6);
    assert_eq!(Preedit::marked("きょう", "", 99).cursor(), 9);
    assert!(Preedit::marked("", "", 0).is_empty());
}
