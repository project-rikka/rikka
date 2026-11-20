use crate::support::{at, developed, flat};
use rikka_image_edit::{Mark, Marks, Recipe, ShapeStyle, Tool};
use rikka_types::{Color, Point};

fn drawn(tool: Tool, from: Point, to: Point) -> Mark {
    let mut mark = Mark::new(tool, Color::rgba(1.0, 0.0, 0.0, 1.0), 6.0, from);
    mark.extend_to(to);
    mark
}

#[test]
fn stroke_paints_only_along_path() {
    let white = flat(40, 40, [255, 255, 255, 255]);
    let line = drawn(Tool::Pen, Point::new(4.0, 20.0), Point::new(36.0, 20.0));
    let marked = developed(&white, Recipe::new().then(Marks::new([line])));
    assert_eq!(at(&marked, 20, 20), [255, 0, 0, 255]);
    assert_eq!(at(&marked, 20, 2), [255, 255, 255, 255]);
}

#[test]
fn hollow_shapes_leave_interior_untouched() {
    let white = flat(60, 60, [255, 255, 255, 255]);
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let shape = drawn(tool, Point::new(10.0, 10.0), Point::new(50.0, 50.0));
        let marked = developed(&white, Recipe::new().then(Marks::new([shape])));
        assert_eq!(at(&marked, 30, 30), [255, 255, 255, 255], "{tool:?}");
        assert_eq!(at(&marked, 30, 10)[1], 0, "{tool:?} draws its top edge");
    }
}

#[test]
fn filled_shapes_cover_interior() {
    let white = flat(60, 60, [255, 255, 255, 255]);
    let blue = Color::rgba(0.0, 0.0, 1.0, 1.0);
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let solid =
            drawn(tool, Point::new(10.0, 10.0), Point::new(50.0, 50.0)).styled(ShapeStyle::Fill);
        let marked = developed(&white, Recipe::new().then(Marks::new([solid])));
        assert_eq!(
            at(&marked, 30, 30),
            [255, 0, 0, 255],
            "{tool:?} is filled with its ink"
        );
        assert_eq!(
            at(&marked, 30, 5),
            [255, 255, 255, 255],
            "{tool:?} stays inside"
        );

        let framed = drawn(tool, Point::new(10.0, 10.0), Point::new(50.0, 50.0))
            .styled(ShapeStyle::FillAndOutline { fill: blue });
        let marked = developed(&white, Recipe::new().then(Marks::new([framed])));
        assert_eq!(
            at(&marked, 30, 30),
            [0, 0, 255, 255],
            "{tool:?} is filled with its fill"
        );
        assert_eq!(
            at(&marked, 30, 10),
            [255, 0, 0, 255],
            "{tool:?} is outlined in its ink"
        );
    }
}
