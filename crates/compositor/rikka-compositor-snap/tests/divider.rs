use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_snap::{Along, between, pulled, shift};
use rikka_compositor_tiling::Edges;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

fn half_and_stack() -> [Rect<i32, Logical>; 3] {
    [
        rect(0, 40, 960, 1040),
        rect(960, 40, 960, 520),
        rect(960, 560, 960, 520),
    ]
}

const FREE: [Size<i32, Logical>; 0] = [];

#[test]
fn shared_line_is_found_under_the_pointer() {
    let rects = half_and_stack();
    let upright = between(&rects, Point::from((962.0, 900.0)), 8.0).expect("line under pointer");
    assert_eq!(
        (upright.along, upright.at, upright.from, upright.to),
        (Along::Vertical, 960, 40, 1080)
    );
    let level = between(&rects, Point::from((1400.0, 556.0)), 8.0).expect("line under pointer");
    assert_eq!(
        (level.along, level.at, level.from, level.to),
        (Along::Horizontal, 560, 960, 1920)
    );
    assert_eq!(between(&rects, Point::from((400.0, 560.0)), 8.0), None);
    assert_eq!(between(&rects, Point::from((980.0, 300.0)), 8.0), None);

    let apart = [rect(0, 0, 900, 800), rect(920, 0, 900, 800)];
    assert_eq!(between(&apart, Point::from((910.0, 400.0)), 8.0), None);
}

#[test]
fn every_window_on_the_line_moves_with_it() {
    let rects = half_and_stack();
    let upright = pulled(&rects, 0, Edges::RIGHT).expect("left window's right edge is shared");
    assert_eq!(pulled(&rects, 1, Edges::LEFT), Some(upright));
    assert_eq!(pulled(&rects, 0, Edges::LEFT), None);
    assert_eq!(upright.takes(Point::from((99.6, -30.0))), 100);

    let moved = shift(&rects, &FREE, upright, 100);
    assert_eq!(
        moved,
        vec![
            Some(rect(0, 40, 1060, 1040)),
            Some(rect(1060, 40, 860, 520)),
            Some(rect(1060, 560, 860, 520)),
        ]
    );

    let level = pulled(&rects, 2, Edges::TOP).expect("stacked windows share a line");
    assert_eq!(
        shift(&rects, &FREE, level, -60),
        vec![
            None,
            Some(rect(960, 40, 960, 460)),
            Some(rect(960, 500, 960, 580)),
        ]
    );
}

#[test]
fn smallest_sizes_stop_the_line() {
    let rects = half_and_stack();
    let upright = pulled(&rects, 0, Edges::RIGHT).expect("shared line");
    let minima = [
        Size::from((700, 300)),
        Size::from((800, 200)),
        Size::from((400, 200)),
    ];
    let pushed = shift(&rects, &minima, upright, 500);
    assert_eq!(pushed[0], Some(rect(0, 40, 1120, 1040)));
    assert_eq!(pushed[1], Some(rect(1120, 40, 800, 520)));
    let pulled_back = shift(&rects, &minima, upright, -500);
    assert_eq!(pulled_back[0], Some(rect(0, 40, 700, 1040)));
    assert_eq!(pulled_back[2], Some(rect(700, 560, 1220, 520)));
}
