use rikka_figure::{CanvasTransform, Form, Frame, Lettering, Look, Pen, Shadow, Shape, recipe};
use rikka_image::ColorType;
use rikka_image_edit::{Depth, Imaging, Picture};
use rikka_types::{Color, Point, Rect};

const SIDE: u32 = 200;

fn drawn(imaging: &Imaging, forms: &[Form], placing: CanvasTransform) -> Picture {
    imaging
        .blank(SIDE, SIDE, Depth::Eight, Color::TRANSPARENT)
        .expect("blank image")
        .develop(&recipe(forms, placing))
        .expect("failed to draw figures")
}

fn pixels(picture: &Picture) -> Vec<[u8; 4]> {
    picture
        .to_image()
        .expect("failed to read back image")
        .convert(ColorType::Rgba8)
        .as_bytes()
        .as_chunks::<4>()
        .0
        .to_vec()
}

fn at(pixels: &[[u8; 4]], x: u32, y: u32) -> [u8; 4] {
    pixels[(y * SIDE + x) as usize]
}

fn caption(outlines: Vec<Pen>, shadow: Option<Shadow>) -> Form {
    Form::Text(Lettering {
        text: "HI".into(),
        size: 80.0,
        weight: Lettering::BOLD,
        color: Color::WHITE,
        origin: (40, 40),
        outlines,
        shadow,
        ..Lettering::default()
    })
}

#[test]
fn figures_draw_within_measured_bounds() {
    let imaging = Imaging::headless().expect("no Vulkan 1.3 compute device");

    let square = Form::Shape {
        shape: Shape::Rectangle { radius: 0.0 },
        frame: Frame::new(Point::new(100.0, 100.0), 80.0, 60.0),
        look: Look {
            fill: Some(Color::rgb(1.0, 0.0, 0.0)),
            outline: Some(Pen::new(Color::rgb(0.0, 0.0, 1.0), 8.0)),
        },
    };
    let shown = pixels(&drawn(
        &imaging,
        std::slice::from_ref(&square),
        CanvasTransform::IDENTITY,
    ));
    assert_eq!(at(&shown, 100, 100), [255, 0, 0, 255]);
    assert_eq!(at(&shown, 60, 100), [0, 0, 255, 255]);
    assert_eq!(at(&shown, 10, 10)[3], 0);

    let halved = pixels(&drawn(
        &imaging,
        std::slice::from_ref(&square),
        CanvasTransform::new(0.5, Point::new(10.0, 10.0)),
    ));
    assert_eq!(at(&halved, 60, 60), [255, 0, 0, 255]);
    assert_eq!(at(&halved, 100, 100)[3], 0);

    let plain = pixels(&drawn(
        &imaging,
        &[caption(Vec::new(), None)],
        CanvasTransform::IDENTITY,
    ));
    let white = |pixel: &&[u8; 4]| pixel[3] == 255 && pixel[0] > 240;
    let black = |pixel: &&[u8; 4]| pixel[3] == 255 && pixel[0] < 15;
    assert!(plain.iter().any(|pixel| white(&pixel)));
    assert!(!plain.iter().any(|pixel| black(&pixel)));

    let edged = caption(vec![Pen::new(Color::BLACK, 6.0)], None);
    let outlined = pixels(&drawn(
        &imaging,
        std::slice::from_ref(&edged),
        CanvasTransform::IDENTITY,
    ));
    assert!(outlined.iter().any(|pixel| white(&pixel)));
    assert!(outlined.iter().any(|pixel| black(&pixel)));
    let inked = |shown: &[[u8; 4]]| shown.iter().filter(|pixel| pixel[3] > 0).count();
    assert!(inked(&outlined) > inked(&plain));

    let shaded = caption(
        vec![Pen::new(Color::BLACK, 6.0)],
        Some(Shadow {
            color: Color::rgba(0.0, 0.0, 0.0, 1.0),
            offset: Point::new(12.0, 12.0),
            blur: 3.0,
        }),
    );
    let reach = shaded.bounds().inflate(1.0);
    let cast = pixels(&drawn(
        &imaging,
        std::slice::from_ref(&shaded),
        CanvasTransform::IDENTITY,
    ));
    assert!(inked(&cast) > inked(&outlined));
    for y in 0..SIDE {
        for x in 0..SIDE {
            if at(&cast, x, y)[3] > 8 {
                assert!(
                    reach.contains(Point::new(x as f32 + 0.5, y as f32 + 0.5)),
                    "{x},{y} lies outside {reach:?}"
                );
            }
        }
    }
    assert!(Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32).contains_rect(&reach));
}
