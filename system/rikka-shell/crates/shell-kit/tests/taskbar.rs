use rikka_settings::session::appearance::TaskbarPosition;
use rikka_types::{CornerRadii, Point, Rect};
use shell_kit::taskbar::{BORDER, DEPTH, Edge, Shape};

const OUTPUT: Rect = Rect::new(0.0, 0.0, 1920.0, 1080.0);

#[test]
fn desktop_leaves_the_strip_on_its_edge_and_a_border_elsewhere() {
    let cases = [
        (
            TaskbarPosition::Top,
            Rect::new(
                BORDER,
                DEPTH,
                1920.0 - BORDER * 2.0,
                1080.0 - DEPTH - BORDER,
            ),
        ),
        (
            TaskbarPosition::Bottom,
            Rect::new(
                BORDER,
                BORDER,
                1920.0 - BORDER * 2.0,
                1080.0 - DEPTH - BORDER,
            ),
        ),
        (
            TaskbarPosition::Left,
            Rect::new(
                DEPTH,
                BORDER,
                1920.0 - DEPTH - BORDER,
                1080.0 - BORDER * 2.0,
            ),
        ),
        (
            TaskbarPosition::Right,
            Rect::new(
                BORDER,
                BORDER,
                1920.0 - DEPTH - BORDER,
                1080.0 - BORDER * 2.0,
            ),
        ),
    ];
    for (position, desktop) in cases {
        let shape = Shape::new(position);
        assert_eq!(shape.desktop(OUTPUT).rect, desktop, "{position:?}");
        let strip = shape.strip();
        let reserved = [strip.top(), strip.right(), strip.bottom(), strip.left()];
        assert_eq!(reserved.iter().sum::<f32>(), DEPTH, "{position:?}");
        assert!(shape.covers(OUTPUT, strip_point(position)), "{position:?}");
        assert!(!shape.covers(OUTPUT, OUTPUT.center()), "{position:?}");
    }
}

#[test]
fn strip_local_rects_round_trip_through_every_edge() {
    let local = Rect::new(120.0, 4.0, 300.0, 36.0);
    let bounds = Rect::new(10.0, 20.0, 1920.0, 400.0);
    for edge in Edge::ALL {
        let bounds = match edge.is_vertical() {
            true => Rect::new(bounds.x(), bounds.y(), bounds.height(), bounds.width()),
            false => bounds,
        };
        let placed = edge.place(bounds, local);
        assert!(bounds.contains(placed.center()), "{edge:?}");
        assert_eq!(edge.local(bounds, placed), local, "{edge:?}");
    }
}

#[test]
fn near_corners_face_the_display_edge() {
    let near = CornerRadii::new(8.0, 8.0, 0.0, 0.0);
    let cases = [
        (TaskbarPosition::Top, CornerRadii::new(8.0, 8.0, 0.0, 0.0)),
        (
            TaskbarPosition::Bottom,
            CornerRadii::new(0.0, 0.0, 8.0, 8.0),
        ),
        (TaskbarPosition::Left, CornerRadii::new(8.0, 0.0, 0.0, 8.0)),
        (TaskbarPosition::Right, CornerRadii::new(0.0, 8.0, 8.0, 0.0)),
    ];
    for (position, surface) in cases {
        assert_eq!(Edge::new(position).radii(near), surface, "{position:?}");
    }
}

fn strip_point(position: TaskbarPosition) -> Point {
    let centre = OUTPUT.center();
    let inside = DEPTH / 2.0;
    match position {
        TaskbarPosition::Top => Point::new(centre.x, inside),
        TaskbarPosition::Bottom => Point::new(centre.x, OUTPUT.max_y() - inside),
        TaskbarPosition::Left => Point::new(inside, centre.y),
        TaskbarPosition::Right => Point::new(OUTPUT.max_x() - inside, centre.y),
    }
}
