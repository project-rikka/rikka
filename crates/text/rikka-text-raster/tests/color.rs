use crate::support::colr::{ColorGlyph, Line, Paint, colr, cpal};
use crate::support::pixel;
use crate::support::sfnt::{Square, font};
use rikka_text_font::{Font, GlyphId};
use rikka_text_raster::{GlyphImage, GlyphImageKind, GlyphRequest, Rasterizer, Rendered};

const RED: u16 = 0;
const GREEN: u16 = 1;
const BLUE: u16 = 2;
const WHITE: u16 = 3;
const BLACK: u16 = 4;
const FOREGROUND: u16 = 0xFFFF;
const PAD: u8 = 0;
const REPEAT: u8 = 1;
const REFLECT: u8 = 2;
const SOURCE_OVER: u8 = 3;
const DESTINATION_OUT: u8 = 8;
const SCREEN: u8 = 13;

const FULL: u16 = 1;
const CENTER: u16 = 2;
const LEFT_HALF: u16 = 3;

fn squares() -> Vec<Square> {
    vec![
        Square {
            glyph: FULL,
            x_min: 0,
            y_min: 0,
            x_max: 1000,
            y_max: 1000,
        },
        Square {
            glyph: CENTER,
            x_min: 300,
            y_min: 300,
            x_max: 700,
            y_max: 700,
        },
        Square {
            glyph: LEFT_HALF,
            x_min: 0,
            y_min: 0,
            x_max: 500,
            y_max: 1000,
        },
    ]
}

fn line(extend: u8, stops: &[(f32, u16)]) -> Line {
    Line {
        extend,
        stops: stops
            .iter()
            .map(|&(offset, index)| (offset, index, 1.0))
            .collect(),
    }
}

fn glyph(paint: Paint) -> Box<Paint> {
    Box::new(paint)
}

fn color_glyphs() -> Vec<ColorGlyph> {
    let entry = |glyph: u16, paint: Paint| ColorGlyph {
        glyph,
        paint,
        clip: None,
    };
    let red_blue = |extend| line(extend, &[(0.0, RED), (1.0, BLUE)]);
    vec![
        entry(11, Paint::Glyph(FULL, glyph(Paint::Solid(BLUE, 0.5)))),
        entry(
            12,
            Paint::Glyph(
                FULL,
                glyph(Paint::Linear([0, 0, 1000, 0, 0, 1000], red_blue(PAD))),
            ),
        ),
        entry(
            13,
            Paint::Glyph(
                FULL,
                glyph(Paint::Radial(
                    [500, 500, 0, 500, 500, 500],
                    line(PAD, &[(0.0, WHITE), (1.0, BLACK)]),
                )),
            ),
        ),
        entry(
            14,
            Paint::Glyph(
                FULL,
                glyph(Paint::Sweep([500, 500], 0.0, 360.0, red_blue(PAD))),
            ),
        ),
        entry(
            15,
            Paint::Composite(
                glyph(Paint::Glyph(CENTER, glyph(Paint::Solid(GREEN, 1.0)))),
                DESTINATION_OUT,
                glyph(Paint::Glyph(FULL, glyph(Paint::Solid(RED, 1.0)))),
            ),
        ),
        ColorGlyph {
            glyph: 16,
            paint: Paint::Glyph(FULL, glyph(Paint::Solid(RED, 1.0))),
            clip: Some([0, 0, 500, 1000]),
        },
        entry(17, Paint::Glyph(FULL, glyph(Paint::Solid(FOREGROUND, 1.0)))),
        entry(
            18,
            Paint::Translate(
                500,
                0,
                glyph(Paint::Glyph(LEFT_HALF, glyph(Paint::Solid(RED, 1.0)))),
            ),
        ),
        entry(
            19,
            Paint::Layers(vec![
                Paint::Glyph(FULL, glyph(Paint::Solid(RED, 1.0))),
                Paint::Glyph(CENTER, glyph(Paint::Solid(GREEN, 1.0))),
            ]),
        ),
        entry(
            20,
            Paint::Glyph(
                FULL,
                glyph(Paint::Linear([0, 0, 250, 0, 0, 250], red_blue(REPEAT))),
            ),
        ),
        entry(
            21,
            Paint::Glyph(
                FULL,
                glyph(Paint::Linear([0, 0, 250, 0, 0, 250], red_blue(REFLECT))),
            ),
        ),
        entry(22, Paint::ColrGlyph(11)),
        entry(
            23,
            Paint::Composite(
                glyph(Paint::Glyph(FULL, glyph(Paint::Solid(GREEN, 1.0)))),
                SCREEN,
                glyph(Paint::Glyph(FULL, glyph(Paint::Solid(RED, 1.0)))),
            ),
        ),
        entry(
            24,
            Paint::Scale(
                0.5,
                0.5,
                glyph(Paint::Glyph(FULL, glyph(Paint::Solid(RED, 1.0)))),
            ),
        ),
        entry(
            25,
            Paint::Glyph(
                FULL,
                glyph(Paint::Composite(
                    glyph(Paint::Solid(GREEN, 1.0)),
                    SOURCE_OVER,
                    glyph(Paint::Solid(RED, 1.0)),
                )),
            ),
        ),
        entry(26, Paint::Glyph(FULL, glyph(Paint::Solid(GREEN, 1.0)))),
    ]
}

fn color_font() -> Vec<u8> {
    let palette = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
    ];
    let layers = vec![(10u16, vec![(FULL, RED), (CENTER, GREEN)])];
    font(
        &squares(),
        vec![
            (b"COLR", colr(&color_glyphs(), &layers)),
            (b"CPAL", cpal(&palette)),
        ],
    )
}

fn render_with(
    data: &[u8],
    glyph: u16,
    request: impl Fn(GlyphRequest<'_>) -> GlyphRequest<'_>,
) -> GlyphImage {
    let font = Font::new(data).unwrap();
    let mut image = GlyphImage::new();
    let request = request(GlyphRequest::new(GlyphId(glyph), 100.0));
    assert_eq!(
        Rasterizer::new().render(&font, &request, &mut image),
        Ok(Rendered::Image),
        "glyph {glyph}"
    );
    image
}

fn render(data: &[u8], glyph: u16) -> GlyphImage {
    let image = render_with(data, glyph, |request| request);
    assert_eq!(image.kind(), GlyphImageKind::Color, "glyph {glyph}");
    image
}

fn srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        linear * 12.92 * 255.0
    } else {
        (1.055 * linear.powf(1.0 / 2.4) - 0.055) * 255.0
    }
}

fn assert_rgba(image: &GlyphImage, x: u32, y: u32, expected: [f32; 4]) {
    let actual = pixel(image, x, y);
    for (channel, (&value, &expected_value)) in actual.iter().zip(&expected).enumerate() {
        assert!(
            (f32::from(value) - expected_value).abs() <= 1.5,
            "pixel {x},{y} channel {channel}: {actual:?} vs {expected:?}"
        );
    }
}

#[test]
fn colr_v0_layers_paint_in_order() {
    let data = color_font();
    let image = render(&data, 10);
    assert_eq!(
        (image.left(), image.top(), image.width(), image.height()),
        (0, 100, 100, 100)
    );
    assert_rgba(&image, 5, 5, [255.0, 0.0, 0.0, 255.0]);
    assert_rgba(&image, 50, 50, [0.0, 255.0, 0.0, 255.0]);
}

#[test]
fn linear_gradients_interpolate_in_linear_light() {
    let data = color_font();
    let image = render(&data, 12);
    for x in [0u32, 25, 49, 80, 99] {
        let t = (x as f32 + 0.5) / 100.0;
        assert_rgba(&image, x, 30, [srgb(1.0 - t), 0.0, srgb(t), 255.0]);
    }
}

#[test]
fn composite_modes_combine_source_and_backdrop() {
    let data = color_font();
    let cut = render(&data, 15);
    assert_rgba(&cut, 5, 5, [255.0, 0.0, 0.0, 255.0]);
    assert_rgba(&cut, 50, 50, [0.0, 0.0, 0.0, 0.0]);
    let screen = render(&data, 23);
    assert_rgba(&screen, 50, 50, [255.0, 255.0, 0.0, 255.0]);
}

#[test]
fn clip_boxes_and_transforms_bound_paint() {
    let data = color_font();
    let clipped = render(&data, 16);
    assert_eq!(
        (clipped.left(), clipped.width(), clipped.height()),
        (0, 50, 100)
    );
    let moved = render(&data, 18);
    assert_eq!((moved.left(), moved.width()), (50, 50));
    assert_rgba(&moved, 10, 10, [255.0, 0.0, 0.0, 255.0]);
    let scaled = render(&data, 24);
    assert_eq!(
        (scaled.left(), scaled.top(), scaled.width(), scaled.height()),
        (0, 50, 50, 50)
    );
}

fn shifted(request: GlyphRequest<'_>) -> GlyphRequest<'_> {
    request.subpixel_x(0.5)
}

#[test]
fn layers_under_a_partial_clip_keep_its_coverage() {
    let data = color_font();
    let layered = render_with(&data, 25, shifted);
    let plain = render_with(&data, 26, shifted);
    assert!((1..255).contains(&pixel(&plain, 0, 50)[3]));
    for x in [0, 50, plain.width() - 1] {
        assert_eq!(pixel(&layered, x, 50), pixel(&plain, x, 50), "column {x}");
    }
}
