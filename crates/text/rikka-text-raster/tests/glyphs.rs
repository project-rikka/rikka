use crate::support::glyph;
use crate::support::ink;
use crate::support::sfnt::{Square, font};
use rikka_test_fonts::font_bytes;
use rikka_text_font::{Font, GlyphId, Location, OutlineSink, Synthesis, Transform};
use rikka_text_font::{OutlineScratch, Tag};
use rikka_text_raster::PreparedOutline;
use rikka_text_raster::{GlyphImage, GlyphImageKind, GlyphRequest, Rasterizer};

fn render(font: &Font<'_>, request: &GlyphRequest<'_>) -> GlyphImage {
    let mut image = GlyphImage::new();
    assert_eq!(
        Rasterizer::new().render_outline(font, request, &mut image),
        Ok(())
    );
    image
}

#[test]
fn latin_masks_match_scaled_bounds() {
    let data = font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let mut scratch = OutlineScratch::new();
    for character in ['a', 'e', 'g', 'W', 'Q', '@', 'i'] {
        let id = glyph(&font, character);
        let bounds = font
            .glyph_bounds(id, &Location::default(), &mut scratch)
            .unwrap();
        let mut inks = Vec::new();
        for ppem in [12.0f32, 16.0, 24.0, 48.0] {
            let image = render(&font, &GlyphRequest::new(id, ppem));
            let scale = ppem / f32::from(font.units_per_em());
            assert_eq!(image.kind(), GlyphImageKind::Mask);
            let near = |actual: i32, expected: f32| (actual as f32 - expected).abs() <= 1.0;
            assert!(
                near(image.left(), (bounds.x_min * scale).floor()),
                "{character} left at {ppem}"
            );
            assert!(
                near(image.top(), (bounds.y_max * scale).ceil()),
                "{character} top at {ppem}"
            );
            let right = image.left() + image.width() as i32;
            let bottom = image.top() - image.height() as i32;
            assert!(
                near(right, (bounds.x_max * scale).ceil()),
                "{character} right at {ppem}"
            );
            assert!(
                near(bottom, (bounds.y_min * scale).floor()),
                "{character} bottom at {ppem}"
            );
            if ppem >= 24.0 {
                assert_eq!(
                    image.pixels().iter().max(),
                    Some(&255),
                    "{character} at {ppem}"
                );
            }
            inks.push(ink(&image) / f64::from(ppem * ppem));
        }
        for value in &inks {
            assert!(
                (value / inks[3] - 1.0).abs() < 0.05,
                "{character}: {inks:?}"
            );
        }
    }
}

#[test]
fn variable_location_changes_rasterized_weight() {
    let data = font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let id = glyph(&font, 'H');
    let thin = font.location([(Tag::new(b"wght"), 100.0)]);
    let black = font.location([(Tag::new(b"wght"), 900.0)]);
    let light = render(&font, &GlyphRequest::new(id, 24.0).location(&thin));
    let heavy = render(&font, &GlyphRequest::new(id, 24.0).location(&black));
    assert!(ink(&heavy) > 2.0 * ink(&light));
    let symbols = font_bytes("MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf");
    let symbols = Font::new(&symbols).unwrap();
    let home = glyph(&symbols, '\u{e88a}');
    let filled = symbols.location([(Tag::new(b"FILL"), 1.0)]);
    let outlined = render(&symbols, &GlyphRequest::new(home, 24.0));
    let solid = render(&symbols, &GlyphRequest::new(home, 24.0).location(&filled));
    assert!(ink(&solid) > 1.2 * ink(&outlined));
}

const SQUARE: u16 = 1;
const PPEM: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Command {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Default)]
struct Path(Vec<Command>);

impl OutlineSink for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::Move(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::Line(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(Command::Quad(x1, y1, x, y));
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Command::Cubic(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.0.push(Command::Close);
    }
}

impl Path {
    fn bounds(&self) -> [f32; 4] {
        let mut bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for command in &self.0 {
            let (x, y) = match *command {
                Command::Move(x, y) | Command::Line(x, y) => (x, y),
                Command::Quad(_, _, x, y) | Command::Cubic(_, _, _, _, x, y) => (x, y),
                Command::Close => continue,
            };
            bounds = [
                bounds[0].min(x),
                bounds[1].min(y),
                bounds[2].max(x),
                bounds[3].max(y),
            ];
        }
        bounds
    }
}

fn square_font() -> Vec<u8> {
    font(
        &[Square {
            glyph: SQUARE,
            x_min: 200,
            y_min: 0,
            x_max: 600,
            y_max: 500,
        }],
        Vec::new(),
    )
}

fn traced(glyph: u16, synthesis: Synthesis) -> Path {
    let data = square_font();
    let font = Font::new(&data).unwrap();
    let mut prepared = PreparedOutline::new();
    assert_eq!(
        Rasterizer::new().prepare_outline(
            &font,
            GlyphId(glyph),
            &Location::default(),
            &mut prepared
        ),
        Ok(())
    );
    let device = Transform {
        yy: -1.0,
        dx: 10.0,
        dy: 30.0,
        ..Transform::IDENTITY
    };
    let mut path = Path::default();
    assert_eq!(prepared.trace(synthesis, PPEM, &device, &mut path), Ok(()));
    path
}

fn assert_bounds(actual: [f32; 4], expected: [f32; 4]) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| (actual - expected).abs() < 1e-3);
    assert!(close, "{actual:?} is not {expected:?}");
}

#[test]
fn trace_applies_device_transform() {
    let path = traced(SQUARE, Synthesis::NONE);
    assert!(matches!(path.0.first(), Some(Command::Move(..))));
    assert_eq!(path.0.last(), Some(&Command::Close));
    assert_eq!(
        path.0
            .iter()
            .filter(|command| **command == Command::Close)
            .count(),
        1
    );
    assert_bounds(path.bounds(), [14.0, 20.0, 22.0, 30.0]);
}
