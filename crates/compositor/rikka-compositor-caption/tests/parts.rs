use rikka_compositor_caption::{Button, Caption, Part};
use rikka_compositor_geometry::{Point, Size};
use rikka_compositor_tiling::Edges;

fn floating() -> Caption {
    Caption::new(Size::new(400, 300)).with_reach(Caption::REACH)
}

#[test]
fn points_name_the_part_under_them() {
    let cases = [
        ("title", (100.0, 20.0), Some(Part::Title)),
        ("separator", (100.0, 38.5), Some(Part::Title)),
        ("close", (380.0, 20.0), Some(Part::Button(Button::Close))),
        (
            "maximize",
            (330.0, 20.0),
            Some(Part::Button(Button::Maximize)),
        ),
        (
            "minimize",
            (270.0, 20.0),
            Some(Part::Button(Button::Minimize)),
        ),
        ("content", (100.0, 100.0), None),
        ("content edge", (1.0, 299.0), None),
        ("top edge", (100.0, 2.0), Some(Part::Edge(Edges::TOP))),
        ("above", (100.0, -6.0), Some(Part::Edge(Edges::TOP))),
        ("top left", (-3.0, -3.0), Some(Part::Edge(Edges::TOP_LEFT))),
        (
            "top right corner reach",
            (390.0, 2.0),
            Some(Part::Edge(Edges::TOP_RIGHT)),
        ),
        ("left of bar", (2.0, 20.0), Some(Part::Edge(Edges::LEFT))),
        (
            "left of content",
            (-4.0, 150.0),
            Some(Part::Edge(Edges::LEFT)),
        ),
        ("below", (200.0, 305.0), Some(Part::Edge(Edges::BOTTOM))),
        (
            "bottom right",
            (405.0, 305.0),
            Some(Part::Edge(Edges::BOTTOM_RIGHT)),
        ),
        ("beyond reach", (200.0, 309.0), None),
    ];
    let caption = floating();
    for (name, (x, y), expected) in cases {
        assert_eq!(caption.part_at(Point::new(x, y)), expected, "{name}");
    }
}

#[test]
fn unreached_caption_has_no_edges() {
    let caption = Caption::new(Size::new(400, 300));
    assert_eq!(caption.part_at(Point::new(100.0, 2.0)), Some(Part::Title));
    assert_eq!(caption.part_at(Point::new(100.0, -2.0)), None);
    assert_eq!(caption.part_at(Point::new(2.0, 20.0)), Some(Part::Title));
}

#[test]
fn buttons_follow_resizability_and_maximization() {
    let buttons =
        |caption: Caption| -> Vec<Button> { caption.buttons().map(|(button, _)| button).collect() };
    let caption = Caption::new(Size::new(400, 300));
    assert_eq!(
        buttons(caption),
        [Button::Minimize, Button::Maximize, Button::Close]
    );
    assert_eq!(
        buttons(caption.with_maximized(true)),
        [Button::Minimize, Button::Restore, Button::Close]
    );
    assert_eq!(
        buttons(caption.with_resizable(false)),
        [Button::Minimize, Button::Close]
    );
    assert_eq!(
        caption
            .with_resizable(false)
            .part_at(Point::new(330.0, 20.0)),
        Some(Part::Button(Button::Minimize))
    );
}
