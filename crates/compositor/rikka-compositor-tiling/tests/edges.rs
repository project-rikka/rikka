use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_tiling::Edges;

fn tile() -> Rect<i32, Logical> {
    Rect::new((100, 100).into(), (400, 300).into())
}

#[test]
fn near_names_the_edges_within_reach() {
    let cases = [
        ((102.0, 250.0), Edges::LEFT),
        ((497.0, 250.0), Edges::RIGHT),
        ((300.0, 95.0), Edges::TOP),
        ((300.0, 404.0), Edges::BOTTOM),
        ((104.0, 396.0), Edges::BOTTOM_LEFT),
        ((503.0, 97.0), Edges::TOP_RIGHT),
        ((300.0, 250.0), Edges::empty()),
        ((85.0, 250.0), Edges::empty()),
        ((300.0, 420.0), Edges::empty()),
    ];
    for (at, edges) in cases {
        assert_eq!(Edges::near(tile(), Point::from(at), 8.0), edges, "{at:?}");
    }
}

#[test]
fn quadrant_picks_the_nearest_corner() {
    let cases = [
        ((150.0, 150.0), Edges::TOP_LEFT),
        ((450.0, 150.0), Edges::TOP_RIGHT),
        ((150.0, 350.0), Edges::BOTTOM_LEFT),
        ((450.0, 350.0), Edges::BOTTOM_RIGHT),
    ];
    for (at, edges) in cases {
        assert_eq!(Edges::quadrant(tile(), Point::from(at)), edges, "{at:?}");
    }
}
