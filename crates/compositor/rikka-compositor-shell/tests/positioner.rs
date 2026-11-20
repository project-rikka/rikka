use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_protocols::xdg_positioner::{Anchor, ConstraintAdjustment, Gravity};
use rikka_compositor_shell::Positioner;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect<i32, Logical> {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

fn positioner(
    anchor_rect: Rect<i32, Logical>,
    size: (i32, i32),
    anchor: Anchor,
    gravity: Gravity,
    adjustment: ConstraintAdjustment,
) -> Positioner {
    Positioner {
        size: Size::new(size.0, size.1),
        anchor_rect,
        anchor,
        gravity,
        constraint_adjustment: adjustment,
        ..Positioner::default()
    }
}

#[test]
fn placement_follows_anchor_gravity_offset() {
    let anchor_rect = rect(10, 20, 30, 40);
    let corner = positioner(
        anchor_rect,
        (100, 50),
        Anchor::BottomRight,
        Gravity::BottomRight,
        ConstraintAdjustment::empty(),
    );
    assert_eq!(corner.placement(), rect(40, 60, 100, 50));

    let centred = positioner(
        anchor_rect,
        (100, 50),
        Anchor::None,
        Gravity::None,
        ConstraintAdjustment::empty(),
    );
    assert_eq!(centred.placement(), rect(-25, 15, 100, 50));

    let offset = Positioner {
        offset: Point::new(5, -3),
        ..positioner(
            anchor_rect,
            (100, 50),
            Anchor::TopLeft,
            Gravity::TopLeft,
            ConstraintAdjustment::empty(),
        )
    };
    assert_eq!(offset.placement(), rect(-85, -33, 100, 50));
}

#[test]
fn unconstraining_flip_applies() {
    let popup = positioner(
        rect(150, 10, 20, 20),
        (80, 30),
        Anchor::Right,
        Gravity::Right,
        ConstraintAdjustment::FLIP_X,
    );
    assert_eq!(popup.geometry(rect(0, 0, 200, 200)), rect(70, 5, 80, 30));
}

#[test]
fn constrained_flip_reverts() {
    let popup = positioner(
        rect(100, 10, 20, 20),
        (150, 30),
        Anchor::Right,
        Gravity::Right,
        ConstraintAdjustment::FLIP_X,
    );
    assert_eq!(popup.geometry(rect(0, 0, 200, 200)), rect(120, 5, 150, 30));
}

#[test]
fn slide_moves_until_fit() {
    let popup = positioner(
        rect(100, 10, 20, 20),
        (150, 30),
        Anchor::Right,
        Gravity::Right,
        ConstraintAdjustment::FLIP_X | ConstraintAdjustment::SLIDE_X,
    );
    assert_eq!(popup.geometry(rect(0, 0, 200, 200)), rect(50, 5, 150, 30));
}

#[test]
fn resize_clips_to_constraint() {
    let popup = positioner(
        rect(100, 10, 20, 20),
        (150, 30),
        Anchor::Right,
        Gravity::Right,
        ConstraintAdjustment::RESIZE_X,
    );
    assert_eq!(popup.geometry(rect(0, 0, 200, 200)), rect(120, 5, 80, 30));
}
