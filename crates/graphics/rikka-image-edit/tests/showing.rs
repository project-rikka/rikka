use crate::support::screen;
use rikka_image::{ColorType, Image};
use rikka_image_edit::{Brush, BrushStroke, Ink, Marking, Marks, Recipe};
use rikka_types::{Color, Point};

const WIDTH: u32 = 1300;
const HEIGHT: u32 = 900;

fn stroke(color: Color, from: (f32, f32), to: (f32, f32)) -> Marking {
    BrushStroke::from_points(
        Brush::new(14.0),
        Ink::Paint(color),
        [Point::new(from.0, from.1), Point::new(to.0, to.1)],
    )
    .expect("stroke has points")
    .into()
}

fn pattern(color: ColorType) -> Image {
    let pixels: Vec<u8> = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .flat_map(|(x, y)| [(x % 251) as u8, (y % 241) as u8, ((x + y) % 256) as u8, 200])
        .collect();
    Image::from_rgba8(WIDTH, HEIGHT, pixels)
        .expect("valid image")
        .convert(color)
}

#[test]
fn incremental_show_matches_full_conversion() {
    let mut screen = screen();
    for color in [ColorType::Rgba8, ColorType::Rgb8] {
        let first = screen
            .imaging()
            .picture(&pattern(color))
            .expect("valid picture");
        let first_shown = first.show(None).expect("picture shows");
        let first_drawn = screen.drawn(first_shown.frame());
        let whole = first
            .shown_area([0, 0], [WIDTH, HEIGHT])
            .expect("full conversion");
        assert!(first_drawn == screen.drawn(&whole), "{color:?}");
        let second = first
            .develop(&Recipe::new().then(Marks::new([stroke(
                Color::rgba(0.9, 0.2, 0.1, 0.7),
                (480.0, 500.0),
                (540.0, 530.0),
            )])))
            .expect("stroke develops");

        let followed = second.show(Some(&first_shown)).expect("edit shows");
        let unchanged = second.show(Some(&followed)).expect("no further change");
        let whole = second
            .shown_area([0, 0], [WIDTH, HEIGHT])
            .expect("full conversion");

        let drawn = screen.drawn(followed.frame());
        assert!(drawn == screen.drawn(&whole), "{color:?}");
        assert!(drawn != first_drawn, "{color:?}");
        assert!(screen.drawn(unchanged.frame()) == drawn, "{color:?}");
        assert!(
            screen.drawn(first_shown.frame()) == first_drawn,
            "{color:?}"
        );
    }
}
