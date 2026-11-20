use rikka_session::switcher::Rows;

#[test]
fn vertical_navigation_picks_nearest_card() {
    let rows = Rows(vec![
        vec![100.0, 300.0, 500.0, 700.0],
        vec![250.0, 450.0, 650.0],
        vec![400.0],
    ]);
    let cases = [
        ("down from the first card", rows.below(0), Some(4)),
        (
            "down from the last card of a long row",
            rows.below(3),
            Some(6),
        ),
        ("down from the middle row", rows.below(5), Some(7)),
        ("down from the last row", rows.below(7), None),
        ("up from the last row", rows.above(7), Some(5)),
        ("up from the middle row", rows.above(4), Some(1)),
        ("up from the first row", rows.above(2), None),
        ("from past the end", rows.below(8), None),
    ];
    for (what, found, expected) in cases {
        assert_eq!(found, expected, "{what}");
    }
}
