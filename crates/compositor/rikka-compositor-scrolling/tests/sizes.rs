use rikka_compositor_scrolling::{Edge, Side, Sizing};

use crate::common::{laid, strip_with};

#[test]
fn width_cycles_through_presets() {
    let mut strip = strip_with(&[1]);
    let widths: Vec<i32> = (0..3)
        .map(|_| {
            assert!(strip.cycle_width());
            laid(&mut strip)[0].1[2]
        })
        .collect();
    assert_eq!(widths, [650, 320, 485]);
}

#[test]
fn edge_resize_anchors_opposite_edge() {
    let mut right = strip_with(&[1]);
    assert!(right.resize(&1, Edge::Right, 15.0));
    assert_eq!(laid(&mut right), [(1, [10, 10, 500, 780])]);
    right.release();
    right.cycle_width();
    assert_eq!(
        laid(&mut right)[0].1[2],
        650,
        "a dragged width cycles on to the next wider preset"
    );

    let mut left = strip_with(&[1]);
    assert!(left.resize(&1, Edge::Left, -20.0));
    assert_eq!(
        laid(&mut left),
        [(1, [-10, 10, 505, 780])],
        "the view does not chase a column while its edge is held"
    );
    left.release();
    assert_eq!(laid(&mut left), [(1, [10, 10, 505, 780])]);
}

#[test]
fn stack_resize_moves_shared_edge() {
    let mut strip = strip_with(&[1, 2]);
    strip.consume_or_expel(Side::Left);
    strip.insert(3, Some(&2));
    strip.settle();
    strip.consume_or_expel(Side::Left);
    let before = laid(&mut strip);
    assert!(strip.resize(&3, Edge::Top, -40.0));
    strip.release();
    let after = laid(&mut strip);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1].1[3], before[1].1[3] - 40);
    assert_eq!(after[2].1[1], before[2].1[1] - 40);
    assert_eq!(after[2].1[3], before[2].1[3] + 40);
    assert!(!strip.resize(&1, Edge::Top, 10.0));
}

#[test]
fn maximized_window_fills_output() {
    let mut strip = strip_with(&[1, 2]);
    assert!(strip.enlarge(&2, Sizing::Fullscreen));
    assert_eq!(
        laid(&mut strip),
        [(1, [-495, 10, 485, 780]), (2, [0, 0, 1000, 800])]
    );
    assert!(strip.enlarge(&2, Sizing::Normal));
    assert_eq!(
        laid(&mut strip),
        [(1, [10, 10, 485, 780]), (2, [505, 10, 485, 780])]
    );

    let mut stacked = strip_with(&[1, 2]);
    stacked.consume_or_expel(Side::Left);
    stacked.settle();
    assert!(stacked.enlarge(&2, Sizing::Maximized));
    assert_eq!(stacked.active(), Some(2));
    assert_eq!(
        laid(&mut stacked),
        [(1, [-495, 10, 485, 780]), (2, [0, 0, 1000, 800])]
    );
}
