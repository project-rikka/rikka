use rikka_types::{Color, Point, Rect};
use rikka_vector::{
    Area, BlendMode, Canvas, Composite, Contours, FillRule, Gradient, GradientShape, Interpolation,
    Mask, Paint, Path, Pattern, Pixmap, Sampling, Spread, Stencil, Stop, Tiling, Transform,
};

const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);

fn rect(x: f32, y: f32, width: f32, height: f32) -> Path {
    let mut path = Path::new();
    path.add_rect(Rect::new(x, y, width, height));
    path
}

fn pixel(canvas: &Canvas, x: u32, y: u32) -> [u8; 4] {
    let at = (y * canvas.width() + x) as usize * 4;
    canvas.pixels()[at..at + 4].try_into().expect("four bytes")
}

fn ink(canvas: &Canvas) -> f32 {
    let alpha: u32 = canvas
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| u32::from(pixel[3]))
        .sum();
    alpha as f32 / 255.0
}

fn canvas(width: u32, height: u32) -> Canvas {
    let mut canvas = Canvas::new();
    canvas.begin(width, height);
    canvas
}

#[test]
fn fills_cover_the_exact_area_under_either_rule() {
    let mut nested = rect(2.25, 2.5, 20.0, 12.0);
    nested.add_rect(Rect::new(6.0, 5.0, 8.0, 4.0));
    for (rule, area) in [
        (FillRule::NonZero, 240.0),
        (FillRule::EvenOdd, 240.0 - 32.0),
    ] {
        let mut canvas = canvas(32, 20);
        canvas
            .fill(&nested, &Transform::IDENTITY, rule, &Paint::Solid(RED))
            .expect("fills");
        assert!(
            (ink(&canvas) - area).abs() < 0.5,
            "{rule:?}: {}",
            ink(&canvas)
        );
        assert_eq!(pixel(&canvas, 4, 4), [255, 0, 0, 255]);
        let hole = pixel(&canvas, 10, 7)[3];
        assert_eq!(hole, if rule == FillRule::EvenOdd { 0 } else { 255 });
    }
}

#[test]
fn touching_and_overlapping_contours_fill_their_union() {
    let mut path = rect(4.0, 3.0, 10.5, 7.4);
    path.add_rect(Rect::new(4.0, 3.0, 10.5, 7.4));
    path.move_to(Point::new(4.0, 10.4));
    path.line_to(Point::new(4.0, 18.0));
    path.line_to(Point::new(14.5, 18.0));
    path.line_to(Point::new(14.5, 10.4));
    path.close();
    let mut canvas = canvas(20, 20);
    canvas
        .fill(
            &path,
            &Transform::IDENTITY,
            FillRule::NonZero,
            &Paint::Solid(RED),
        )
        .expect("fills");
    assert!((ink(&canvas) - 10.5 * 15.0).abs() < 0.5, "{}", ink(&canvas));
    assert_eq!(
        pixel(&canvas, 8, 10),
        [255, 0, 0, 255],
        "row shared by opposite contours"
    );
    let edge = pixel(&canvas, 14, 8)[3];
    assert!(
        (126..=129).contains(&edge),
        "edge shared by equal contours: {edge}"
    );
}

#[test]
fn transforms_place_and_scale_paths() {
    let mut canvas = canvas(40, 40);
    let transform = Transform::scale(2.0, 3.0).then(Transform::translate(4.0, 5.0));
    canvas
        .fill(
            &rect(1.0, 1.0, 5.0, 5.0),
            &transform,
            FillRule::NonZero,
            &Paint::Solid(BLUE),
        )
        .expect("fills");
    assert!((ink(&canvas) - 150.0).abs() < 0.5);
    assert_eq!(pixel(&canvas, 6, 8), [0, 0, 255, 255]);
    assert_eq!(pixel(&canvas, 5, 8)[3], 0);
    assert_eq!(pixel(&canvas, 15, 22)[3], 255);
    assert_eq!(pixel(&canvas, 16, 22)[3], 0);
}

#[test]
fn clips_bound_layers_and_layers_fade_and_blend() {
    let mut canvas = canvas(20, 20);
    let whole = rect(0.0, 0.0, 20.0, 20.0);
    canvas
        .fill(
            &whole,
            &Transform::IDENTITY,
            FillRule::NonZero,
            &Paint::Solid(BLUE),
        )
        .expect("fills");
    canvas
        .push_clip(
            &rect(5.0, 5.0, 10.0, 10.0),
            &Transform::IDENTITY,
            FillRule::NonZero,
            Contours::Overlapping,
        )
        .expect("clips");
    canvas.push_layer();
    canvas
        .fill(
            &whole,
            &Transform::IDENTITY,
            FillRule::NonZero,
            &Paint::Solid(RED),
        )
        .expect("fills");
    canvas.pop_layer(&Composite::faded(0.5));
    canvas.pop_clip();
    assert_eq!(pixel(&canvas, 2, 2), [0, 0, 255, 255]);
    let mixed = pixel(&canvas, 10, 10);
    assert!(
        (126..=129).contains(&mixed[0]) && (126..=129).contains(&mixed[2]),
        "{mixed:?}"
    );

    canvas.push_layer_within(Area {
        left: 0,
        top: 0,
        width: 4,
        height: 4,
    });
    canvas.paint(&Paint::Solid(RED), &Transform::IDENTITY);
    canvas.pop_layer(&Composite::blending(BlendMode::Screen));
    assert_eq!(pixel(&canvas, 1, 1), [255, 0, 255, 255]);
    assert_eq!(pixel(&canvas, 4, 4), [0, 0, 255, 255]);
}

#[test]
fn layers_beside_the_canvas_leave_it_untouched() {
    let mut canvas = canvas(16, 16);
    for area in [
        Area {
            left: 30,
            top: 4,
            width: 8,
            height: 8,
        },
        Area {
            left: 4,
            top: -20,
            width: 8,
            height: 8,
        },
    ] {
        canvas.push_layer_within(area);
        canvas.paint(&Paint::Solid(RED), &Transform::IDENTITY);
        canvas.pop_layer(&Composite::OVER);
    }
    assert_eq!(ink(&canvas), 0.0);
}

#[test]
fn stencils_weight_clips_and_layers() {
    let mut half = Pixmap::new(4, 4).expect("pixmap");
    for pixel in half.data_mut().as_chunks_mut::<4>().0 {
        *pixel = [0, 0, 0, 128];
    }
    let mask = Mask::from_alpha(&half);
    let stencil = Stencil {
        mask: &mask,
        left: 2,
        top: 2,
    };

    let mut clipped = canvas(8, 8);
    clipped.push_clip_stencil(stencil);
    clipped.paint(&Paint::Solid(RED), &Transform::IDENTITY);
    clipped.pop_clip();
    assert_eq!(pixel(&clipped, 3, 3)[3], 128);
    assert_eq!(pixel(&clipped, 1, 1)[3], 0);

    let mut layered = canvas(8, 8);
    layered.push_layer();
    layered.paint(&Paint::Solid(RED), &Transform::IDENTITY);
    layered.pop_layer(&Composite {
        mask: Some(stencil),
        ..Composite::OVER
    });
    assert_eq!(pixel(&layered, 3, 3)[3], 128);
    assert_eq!(pixel(&layered, 6, 6)[3], 0);
}

#[test]
fn gradients_interpolate_in_their_space_and_spread() {
    let stops = [
        Stop {
            offset: 0.0,
            color: RED,
        },
        Stop {
            offset: 1.0,
            color: BLUE,
        },
    ];
    let linear = |spread, interpolation| Gradient {
        shape: GradientShape::Linear {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
        },
        stops: &stops,
        spread,
        interpolation,
        transform: Transform::IDENTITY,
    };
    let render = |gradient: Gradient<'_>| {
        let mut canvas = canvas(30, 2);
        canvas.paint(&Paint::Gradient(gradient), &Transform::IDENTITY);
        canvas
    };

    let padded = render(linear(Spread::Pad, Interpolation::Srgb));
    assert_eq!(pixel(&padded, 5, 0), [115, 0, 140, 255]);
    assert_eq!(pixel(&padded, 25, 0), [0, 0, 255, 255]);

    let light = render(linear(Spread::Pad, Interpolation::LinearLight));
    let middle = pixel(&light, 5, 0);
    assert!(middle[0] > 160 && middle[2] > 180, "{middle:?}");

    let repeated = render(linear(Spread::Repeat, Interpolation::Srgb));
    assert_eq!(pixel(&repeated, 15, 0), pixel(&repeated, 5, 0));
    let reflected = render(linear(Spread::Reflect, Interpolation::Srgb));
    assert_eq!(pixel(&reflected, 14, 0), pixel(&reflected, 5, 0));

    let radial = render(Gradient {
        shape: GradientShape::Radial {
            start_center: Point::new(15.0, 1.0),
            start_radius: 0.0,
            end_center: Point::new(15.0, 1.0),
            end_radius: 10.0,
        },
        ..linear(Spread::Pad, Interpolation::Srgb)
    });
    assert!(pixel(&radial, 15, 1)[0] > 230);
    assert_eq!(pixel(&radial, 28, 1), [0, 0, 255, 255]);
}

#[test]
fn patterns_tile_or_stop_at_their_edge() {
    let mut tile = Pixmap::new(2, 1).expect("pixmap");
    tile.data_mut()
        .copy_from_slice(&[255, 0, 0, 255, 0, 0, 255, 255]);
    let pattern = |tiling| Pattern {
        pixmap: &tile,
        transform: Transform::scale(2.0, 2.0),
        tiling,
        sampling: Sampling::Nearest,
        opacity: 1.0,
    };
    let render = |tiling| {
        let mut canvas = canvas(12, 2);
        canvas.paint(&Paint::Pattern(pattern(tiling)), &Transform::IDENTITY);
        canvas
    };
    let once = render(Tiling::Once);
    assert_eq!(pixel(&once, 1, 1), [255, 0, 0, 255]);
    assert_eq!(pixel(&once, 3, 1), [0, 0, 255, 255]);
    assert_eq!(pixel(&once, 5, 1)[3], 0);
    let repeated = render(Tiling::Repeat);
    assert_eq!(pixel(&repeated, 5, 1), [255, 0, 0, 255]);
    assert_eq!(pixel(&repeated, 11, 1), [0, 0, 255, 255]);
}
