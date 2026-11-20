use desk_layout::{Area, GAP, Size, first_vacant, overlaps, vacant};
use rikka_types::Rect;

#[test]
fn free_spot_is_kept() {
    let area = Area::for_desk(Rect::new(0.0, 0.0, 1200.0, 800.0));
    let candidate = Rect::new(400.0, 300.0, 100.0, 100.0);
    let others = [Rect::new(100.0, 100.0, 100.0, 100.0)];
    assert_eq!(vacant(candidate, &others, &area), candidate);
}

#[test]
fn occupied_spot_moves_to_nearest_free() {
    let area = Area::for_desk(Rect::new(0.0, 0.0, 1200.0, 800.0));
    let others = [
        Rect::new(100.0, 100.0, 352.0, 168.0),
        Rect::new(468.0, 100.0, 168.0, 168.0),
        Rect::new(100.0, 284.0, 168.0, 352.0),
    ];
    for candidate in [
        Rect::new(120.0, 120.0, 168.0, 168.0),
        Rect::new(470.0, 110.0, 352.0, 168.0),
        Rect::new(1100.0, 700.0, 352.0, 352.0),
        Rect::new(150.0, 300.0, 168.0, 168.0),
    ] {
        let found = vacant(candidate, &others, &area);
        assert!(
            !overlaps(found, &others),
            "{candidate:?} landed on {found:?}"
        );
        assert!(area.room().inflate(0.01).contains_rect(&found));
        assert_eq!(found.size, candidate.size);
    }
}

#[test]
fn new_widgets_fill_first_free_spot() {
    let area = Area::for_desk(Rect::new(0.0, 0.0, 1200.0, 800.0));
    let room = area.room();
    let small = Size::Small.extent();
    let cases: [(&str, Vec<Rect>, Option<Rect>); 4] = [
        (
            "an empty desk starts at the top left",
            vec![],
            Some(Rect::new(
                room.min_x(),
                room.min_y(),
                small.width,
                small.height,
            )),
        ),
        (
            "beside the last one, a gap apart",
            vec![Rect::new(room.min_x(), room.min_y(), 352.0, 168.0)],
            Some(Rect::new(
                room.min_x() + 352.0 + GAP,
                room.min_y(),
                small.width,
                small.height,
            )),
        ),
        (
            "a full row moves on to the next",
            vec![Rect::new(room.min_x(), room.min_y(), room.width(), 168.0)],
            Some(Rect::new(
                room.min_x(),
                room.min_y() + 168.0 + GAP,
                small.width,
                small.height,
            )),
        ),
        ("a full desk has no place", vec![room], None),
    ];
    for (case, others, expected) in cases {
        assert_eq!(first_vacant(small, &others, &area), expected, "{case}");
    }
}
