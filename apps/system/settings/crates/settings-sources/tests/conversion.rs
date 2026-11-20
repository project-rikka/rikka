use rikka_ime::{DEFAULT_LAYOUT, Group, InputSource, Member};
use settings_sources::{group_from_sources, sources_from_group};

fn group() -> Group {
    Group {
        name: "Default".to_owned(),
        default_layout: "us".to_owned(),
        members: Vec::new(),
    }
}

#[test]
fn sources_round_trip_through_group() {
    let sources = vec![
        InputSource::layout("us"),
        InputSource::layout("us").variant("intl"),
        InputSource::layout("jp").engine("mozc"),
        InputSource::layout("de").variant("neo").engine("rime"),
    ];

    let group_back = group_from_sources(&group(), &sources);
    assert_eq!(sources_from_group(&group_back), sources);
}

#[test]
fn plain_layout_uses_keyboard_engine() {
    let group_back = group_from_sources(&group(), &[InputSource::layout("us").variant("intl")]);

    assert_eq!(
        group_back.members,
        [Member::new("keyboard-us-intl").with_layout("us-intl")]
    );
}

#[test]
fn member_without_layout_uses_engine_name() {
    let mut input = group();
    input.members = vec![Member::new("keyboard-jp"), Member::new("mozc")];

    assert_eq!(
        sources_from_group(&input),
        [
            InputSource::layout("jp"),
            InputSource::layout("us").engine("mozc"),
        ]
    );
}

#[test]
fn group_round_trips_every_member() {
    let mut input = group();
    input.members = vec![
        Member::new("keyboard-us"),
        Member::new("mozc"),
        Member::new("pinyin").with_layout("us-intl"),
    ];

    let group_back = group_from_sources(&input, &sources_from_group(&input));

    assert_eq!(sources_from_group(&group_back), sources_from_group(&input));
    assert_eq!(group_back.default_layout, "us");
}

#[test]
fn empty_default_layout_falls_back_to_us() {
    let input = Group {
        name: "Default".to_owned(),
        default_layout: String::new(),
        members: vec![Member::new("mozc")],
    };

    assert_eq!(
        sources_from_group(&input),
        [InputSource::layout(DEFAULT_LAYOUT).engine("mozc")]
    );
    assert_eq!(
        group_from_sources(&input, &sources_from_group(&input)).default_layout,
        DEFAULT_LAYOUT
    );
}
