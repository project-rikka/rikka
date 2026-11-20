use rikka_compositor_scrolling::Direction;

use crate::common::{corners, frame, laid, strip_with};

type Placed = (u32, [i32; 4]);

#[test]
fn new_window_opens_beside_focus() {
    let cases: [(&[u32], &[Placed]); 3] = [
        (&[1], &[(1, [10, 10, 485, 780])]),
        (
            &[1, 2],
            &[(1, [10, 10, 485, 780]), (2, [505, 10, 485, 780])],
        ),
        (
            &[1, 2, 3],
            &[
                (1, [-485, 10, 485, 780]),
                (2, [10, 10, 485, 780]),
                (3, [505, 10, 485, 780]),
            ],
        ),
    ];
    for (windows, expected) in cases {
        let mut strip = strip_with(windows);
        assert_eq!(laid(&mut strip), expected, "after opening {windows:?}");
        assert_eq!(strip.active(), windows.last().copied());
    }
}

#[test]
fn window_opens_at_announced_position() {
    let mut strip = strip_with(&[1, 2]);
    strip.focus(Direction::Left);
    strip.settle();
    let foretold = strip.arrival(frame(), Some(&1));
    strip.insert(3, Some(&1));
    let landed = laid(&mut strip);
    assert_eq!(
        landed
            .iter()
            .find(|(window, _)| *window == 3)
            .map(|(_, at)| *at),
        Some(corners(foretold))
    );
    assert_eq!(
        landed.iter().map(|(window, _)| *window).collect::<Vec<_>>(),
        [1, 3, 2]
    );
}

#[test]
fn focus_scrolls_column_minimally() {
    let mut strip = strip_with(&[1, 2, 3]);
    assert_eq!(strip.focus(Direction::Left), Some(2));
    assert_eq!(laid(&mut strip)[1], (2, [10, 10, 485, 780]));
    assert_eq!(strip.focus(Direction::Left), Some(1));
    assert_eq!(
        laid(&mut strip),
        [
            (1, [10, 10, 485, 780]),
            (2, [505, 10, 485, 780]),
            (3, [1000, 10, 485, 780]),
        ]
    );
    assert_eq!(strip.focus(Direction::Left), None);
}

#[test]
fn closing_column_restores_previous_focus() {
    let mut fresh = strip_with(&[1, 2, 3]);
    assert!(fresh.remove(&3));
    assert_eq!(fresh.active(), Some(2));
    assert_eq!(
        laid(&mut fresh),
        [(1, [10, 10, 485, 780]), (2, [505, 10, 485, 780])],
        "closing the column just opened restores the view it was opened from"
    );

    let mut visited = strip_with(&[1, 2, 3]);
    visited.focus(Direction::Left);
    visited.settle();
    assert!(visited.remove(&2));
    assert_eq!(
        visited.active(),
        Some(3),
        "the column to the right takes the place of a closed one"
    );
    assert_eq!(
        laid(&mut visited),
        [(1, [-485, 10, 485, 780]), (3, [10, 10, 485, 780])]
    );

    let mut moved = strip_with(&[1, 2, 3]);
    moved.shift(Direction::Left);
    moved.settle();
    assert!(moved.remove(&3));
    assert_eq!(
        moved.active(),
        Some(2),
        "a column moved after opening no longer hands focus back to where it was opened"
    );
}
