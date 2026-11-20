use rikka_compositor_geometry::Point;
use rikka_compositor_scrolling::{Direction, Side};

use crate::common::{laid, strip_with};

#[test]
fn consume_and_expel_windows() {
    let mut strip = strip_with(&[1, 2]);
    assert!(strip.consume_or_expel(Side::Left));
    assert_eq!(strip.active(), Some(2));
    assert_eq!(
        laid(&mut strip),
        [(1, [10, 10, 485, 385]), (2, [10, 405, 485, 385])]
    );

    assert!(strip.consume_or_expel(Side::Right));
    assert_eq!(strip.active(), Some(2));
    assert_eq!(
        laid(&mut strip),
        [(1, [10, 10, 485, 780]), (2, [505, 10, 485, 780])]
    );
}

#[test]
fn edge_columns_cannot_consume_outward() {
    let mut strip = strip_with(&[1, 2]);
    assert!(!strip.consume_or_expel(Side::Right));
    strip.focus(Direction::Left);
    assert!(!strip.consume_or_expel(Side::Left));
}

#[test]
fn swapping_keeps_view_offset() {
    let mut strip = strip_with(&[1, 2, 3]);
    assert!(strip.shift(Direction::Left));
    assert_eq!(strip.active(), Some(3));
    assert_eq!(
        laid(&mut strip),
        [
            (1, [-485, 10, 485, 780]),
            (3, [10, 10, 485, 780]),
            (2, [505, 10, 485, 780]),
        ]
    );

    let mut stacked = strip_with(&[1, 2]);
    stacked.consume_or_expel(Side::Left);
    assert!(stacked.shift(Direction::Up));
    assert!(!stacked.shift(Direction::Up));
    assert_eq!(
        laid(&mut stacked),
        [(2, [10, 10, 485, 385]), (1, [10, 405, 485, 385])]
    );
    assert_eq!(stacked.focus(Direction::Down), Some(1));
}

#[test]
fn dropped_window_swaps_or_inserts_column() {
    let mut onto = strip_with(&[1, 2]);
    assert!(onto.drop_at(&1, Point::new(700.0, 400.0)));
    assert_eq!(
        laid(&mut onto),
        [(2, [10, 10, 485, 780]), (1, [505, 10, 485, 780])]
    );

    let mut beyond = strip_with(&[1, 2]);
    beyond.consume_or_expel(Side::Left);
    beyond.settle();
    assert!(beyond.drop_at(&2, Point::new(999.0, 400.0)));
    assert_eq!(beyond.active(), Some(2));
    assert_eq!(
        laid(&mut beyond),
        [(1, [10, 10, 485, 780]), (2, [505, 10, 485, 780])]
    );
}
