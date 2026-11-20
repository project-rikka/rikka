use rikka_blur::opaque_rectangles;
use rikka_types::{CornerRadii, Rect, RoundedRect};

fn covered(areas: &[[i32; 4]], x: i32, y: i32) -> bool {
    areas.iter().any(|[left, top, width, height]| {
        x >= *left && x < left + width && y >= *top && y < top + height
    })
}

fn window(radius: f32) -> RoundedRect {
    RoundedRect::new(
        Rect::new(0.0, 0.0, 800.0, 600.0),
        CornerRadii::uniform(radius),
    )
}

type Case = (
    &'static str,
    Vec<RoundedRect>,
    Vec<RoundedRect>,
    Vec<(i32, i32)>,
    Vec<(i32, i32)>,
);

#[test]
fn only_unblurred_opaque_pixels_are_opaque() {
    let reach = (12.0 * RoundedRect::CORNER_REACH).ceil() as i32;
    let pane = RoundedRect::new(
        Rect::new(100.0, 100.0, 200.0, 50.0),
        CornerRadii::uniform(0.0),
    );
    let cases: [Case; 5] = [
        (
            "a square window is opaque edge to edge",
            vec![window(0.0)],
            Vec::new(),
            vec![(0, 0), (799, 0), (0, 599), (799, 599), (400, 300)],
            vec![(800, 300), (-1, 0), (400, 600)],
        ),
        (
            "a rounded window leaves each corner's whole reach out",
            vec![window(12.0)],
            Vec::new(),
            vec![(reach, 0), (0, reach), (799 - reach, 599), (400, 300)],
            vec![
                (0, 0),
                (reach - 1, reach - 1),
                (799, 599),
                (800 - reach, 600 - reach),
            ],
        ),
        (
            "a frosted pane is cut out of the opaque window",
            vec![window(0.0)],
            vec![pane],
            vec![(99, 120), (300, 120), (150, 99), (150, 150)],
            vec![(100, 100), (299, 149), (200, 120)],
        ),
        (
            "a shape between pixels keeps only the pixels it fully covers",
            vec![RoundedRect::new(
                Rect::new(0.5, 0.5, 10.0, 10.0),
                CornerRadii::uniform(0.0),
            )],
            Vec::new(),
            vec![(1, 1), (9, 9)],
            vec![(0, 0), (10, 10), (0, 5)],
        ),
        (
            "nothing painted opaque declares nothing",
            Vec::new(),
            vec![pane],
            Vec::new(),
            vec![(150, 120), (0, 0)],
        ),
    ];
    for (name, opaque, glass, inside, outside) in cases {
        let mut areas = Vec::new();
        opaque_rectangles(&opaque, &glass, &mut areas);
        for (x, y) in inside {
            assert!(covered(&areas, x, y), "{name}: ({x}, {y}) should be opaque");
        }
        for (x, y) in outside {
            assert!(
                !covered(&areas, x, y),
                "{name}: ({x}, {y}) should not be opaque"
            );
        }
    }
}
