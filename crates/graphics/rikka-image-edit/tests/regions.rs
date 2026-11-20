use crate::support::{imaging, picture, rgba};
use rikka_image::{ColorType, Image};
use rikka_image_edit::{
    Blend, Brush, BrushStroke, Crop, Fill, FillRule, GaussianBlur, Ink, Marking, Marks, Mask, Path,
    Picture, Recipe, Span, Step,
};
use rikka_types::{Color, Point};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;

fn shaded() -> Picture {
    picture(WIDTH, HEIGHT, |x, y| {
        [
            (x * 7 % 256) as u8,
            (y * 5 % 256) as u8,
            ((x + y) % 256) as u8,
            (96 + x % 150) as u8,
        ]
    })
}

fn plain(color: ColorType) -> Picture {
    let image =
        Image::filled(WIDTH, HEIGHT, color, Color::rgb(0.3, 0.6, 0.45)).expect("valid image");
    imaging().picture(&image).expect("valid picture")
}

fn stroke(ink: Ink, points: &[(f32, f32)]) -> Marking {
    let point = |(x, y): (f32, f32)| Point::new(x, y);
    BrushStroke::from_points(
        Brush {
            hardness: 0.4,
            ..Brush::new(9.0)
        },
        ink,
        points.iter().copied().map(point),
    )
    .expect("stroke has points")
    .into()
}

fn whole(recipe: &Recipe) -> Recipe {
    recipe
        .clone()
        .then(Step::new(Crop::new(0, 0, WIDTH, HEIGHT)))
}

#[test]
fn windowed_develop_matches_full_develop() {
    let brushed = Marks::new([stroke(
        Ink::Paint(Color::rgba(0.9, 0.1, 0.2, 0.8)),
        &[(200.0, 150.0), (230.0, 160.0), (250.0, 120.0)],
    )]);
    let erased = Marks::new([stroke(
        Ink::Erase {
            background: Color::WHITE,
        },
        &[(40.0, 30.0), (70.0, 45.0)],
    )]);
    let selection = Mask::spans(
        WIDTH,
        HEIGHT,
        (60..90).map(|y| Span {
            y,
            from: 100 + y % 7,
            to: 150,
        }),
    )
    .expect("selection rects");
    let triangle = Path::builder()
        .move_to(Point::new(250.0, 20.0))
        .line_to(Point::new(300.0, 60.0))
        .line_to(Point::new(240.0, 70.0))
        .close()
        .build();
    let cases: [(&str, Recipe); 8] = [
        (
            "blur in a selection",
            Recipe::new().then(Step::new(GaussianBlur::new(3.0)).masked(selection.clone())),
        ),
        (
            "fill in a feathered ellipse",
            Recipe::new().then(
                Step::new(Fill::new(Color::rgba(0.2, 0.9, 0.3, 0.9)))
                    .masked(Mask::ellipse(150.0, 20.0, 40.0, 30.0).feathered(6.0)),
            ),
        ),
        ("brush", Recipe::new().then(brushed.clone())),
        ("eraser", Recipe::new().then(erased)),
        (
            "fill in a selection",
            Recipe::new().then(
                Step::new(Fill::new(Color::rgba(0.1, 0.2, 0.9, 0.7))).masked(selection.clone()),
            ),
        ),
        (
            "brush in a selection",
            Recipe::new().then(Step::new(brushed.clone()).masked(selection)),
        ),
        (
            "dissolved fill in a path",
            Recipe::new().then(
                Step::new(Fill::new(Color::rgb(1.0, 0.8, 0.0)))
                    .masked(Mask::path(triangle, FillRule::NonZero))
                    .opacity(0.5)
                    .blend(Blend::Dissolve),
            ),
        ),
        (
            "fill in an ellipse after a brush",
            Recipe::new().then(brushed).then(
                Step::new(Fill::new(Color::BLACK)).masked(Mask::ellipse(20.5, 120.25, 33.0, 21.5)),
            ),
        ),
    ];
    for source in [shaded(), plain(ColorType::Rgb8), plain(ColorType::Rgb16)] {
        for (name, recipe) in &cases {
            let windowed = source.develop(recipe).expect("change develops");
            let reference = source
                .develop(&whole(recipe))
                .expect("full develop succeeds");
            assert!(
                rgba(&windowed) == rgba(&reference),
                "{name} on a {:?} picture differs from developing the whole picture",
                source.depth()
            );
            assert_eq!(windowed.has_alpha(), reference.has_alpha(), "{name}");
        }
    }
}

#[test]
fn edit_across_tile_seams_keeps_earlier_versions() {
    let (width, height) = (1300, 900);
    let first = picture(width, height, |x, y| {
        [
            (x * 7 % 256) as u8,
            (y * 5 % 256) as u8,
            ((x ^ y) % 256) as u8,
            255,
        ]
    });
    let strokes = [
        stroke(
            Ink::Paint(Color::rgba(0.9, 0.1, 0.2, 0.8)),
            &[(500.0, 500.0), (530.0, 520.0), (560.0, 490.0)],
        ),
        stroke(
            Ink::Erase {
                background: Color::WHITE,
            },
            &[(1010.0, 30.0), (1040.0, 60.0)],
        ),
        stroke(
            Ink::Paint(Color::rgba(0.1, 0.8, 0.3, 1.0)),
            &[(20.0, 880.0), (1280.0, 20.0)],
        ),
    ];
    let mut versions = vec![(first.clone(), rgba(&first))];
    for marking in strokes {
        let recipe = Recipe::new().then(Marks::new([marking]));
        let latest = versions
            .last()
            .map(|(picture, _)| picture.clone())
            .expect("version present");
        let windowed = latest.develop(&recipe).expect("stroke develops");
        let reference = latest
            .develop(
                &recipe
                    .clone()
                    .then(Step::new(Crop::new(0, 0, width, height))),
            )
            .expect("full develop succeeds");
        let pixels = rgba(&windowed);
        assert!(
            pixels == rgba(&reference),
            "a stroke differs from developing the whole picture"
        );
        versions.push((windowed, pixels));
    }
    let (last, _) = versions.last().expect("version present").clone();
    let area = last
        .changed_area(&first)
        .expect("pictures compare")
        .expect("pictures differ");
    let restored = last.patched(&first.piece(area).expect("piece cut"), [area[0], area[1]]);
    assert!(rgba(&restored.expect("piece fits")) == versions[0].1);
    for (index, (picture, pixels)) in versions.iter().enumerate() {
        assert!(
            rgba(picture) == *pixels,
            "version {index} changed after later edits"
        );
    }
}
