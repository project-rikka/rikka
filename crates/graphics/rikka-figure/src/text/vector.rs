use rikka_image_edit::{Path, PathBuilder, Stamp};
use rikka_text::{Glyph, GlyphImage, GlyphKey, GlyphKind, GlyphPath, GlyphRaster};
use rikka_types::{Color, Point, Rect};

use super::raster::with_glyphs;
use super::typeset::Typeset;
use crate::affine::Affine;
use crate::frame::enclosing;

#[derive(Clone, Debug)]
pub(crate) struct Letterform {
    pub(crate) outline: Option<Path>,
    pub(crate) colored: Vec<(Glyph, Point)>,
    pub(crate) centre: Point,
    pub(crate) frame: Rect,
    pub(crate) ink: Rect,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Turn {
    pub(crate) centre: Point,
    pub(crate) degrees: f32,
}

#[derive(Default)]
struct Builder(Option<PathBuilder>);

impl Builder {
    fn step(&mut self, step: impl FnOnce(PathBuilder) -> PathBuilder) {
        let builder = self.0.take().unwrap_or_else(Path::builder);
        self.0 = Some(step(builder));
    }
}

impl GlyphPath for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.step(|path| path.move_to(Point::new(x, y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.step(|path| path.line_to(Point::new(x, y)));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.step(|path| path.quad_to(Point::new(x1, y1), Point::new(x, y)));
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.step(|path| path.cubic_to(Point::new(x1, y1), Point::new(x2, y2), Point::new(x, y)));
    }

    fn close(&mut self) {
        self.step(PathBuilder::close);
    }
}

#[derive(Default)]
struct GlyphAccumulator {
    outline: Builder,
    colored: Vec<(Glyph, Point)>,
    advance: Option<Rect>,
}

impl GlyphAccumulator {
    fn finished(self) -> Option<Letterform> {
        let outline = self
            .outline
            .0
            .map(PathBuilder::build)
            .filter(|path| !path.is_empty());
        let drawn = outline.as_ref().map(|path| {
            enclosing(
                path.subpaths()
                    .iter()
                    .flat_map(|subpath| subpath.points().iter().copied()),
            )
        });
        let ink = match (self.advance, drawn) {
            (Some(advance), Some(drawn)) => advance.union(&drawn),
            (Some(only), None) | (None, Some(only)) => only,
            (None, None) => return None,
        };
        let frame = self.advance.unwrap_or(ink);
        Some(Letterform {
            centre: frame.center(),
            outline,
            colored: self.colored,
            frame,
            ink,
        })
    }
}

pub(crate) fn letterforms(typeset: &Typeset, text: &str, scale: f32) -> Vec<Option<Letterform>> {
    let starts: Vec<usize> = text
        .char_indices()
        .filter(|(_, letter)| !letter.is_whitespace())
        .map(|(at, _)| at)
        .collect();
    let Some(last) = starts.len().checked_sub(1) else {
        return Vec::new();
    };
    let mut gathered: Vec<GlyphAccumulator> =
        starts.iter().map(|_| GlyphAccumulator::default()).collect();
    let origin = typeset.origin() * scale;
    with_glyphs(|raster| {
        for row in typeset.laid().rows() {
            let pen = (origin.x, origin.y + row.baseline() * scale);
            let top = origin.y + row.top() * scale;
            let height = row.height() * scale;
            for glyph in row.glyphs() {
                let byte = row.offset_of(glyph);
                let letter = byte.map_or(last, |byte| {
                    starts.partition_point(|start| *start < byte).min(last)
                });
                let blank = byte
                    .and_then(|byte| text.get(byte..))
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(char::is_whitespace);
                let builder = &mut gathered[letter];
                if !blank {
                    let left = pen.0 + glyph.x * scale;
                    let advance = Rect::new(left, top, glyph.advance * scale, height);
                    builder.advance = Some(match builder.advance {
                        Some(before) => before.union(&advance),
                        None => advance,
                    });
                }
                if !raster.outline(glyph, pen, scale, &mut builder.outline) {
                    builder.colored.push((*glyph, Point::new(pen.0, pen.1)));
                }
            }
        }
    });
    gathered
        .into_iter()
        .map(GlyphAccumulator::finished)
        .collect()
}

pub(crate) struct Inking {
    pub(crate) scale: f32,
    pub(crate) color: Color,
    pub(crate) opacity: f32,
    pub(crate) turn: Turn,
}

pub(crate) fn stamp(
    raster: &mut GlyphRaster,
    glyph: &Glyph,
    pen: Point,
    inking: &Inking,
) -> Option<Stamp> {
    let (key, x, y) = GlyphKey::place(glyph, (pen.x, pen.y), inking.scale);
    let image = raster.raster(key)?;
    match inking.turn.degrees.rem_euclid(360.0) == 0.0 {
        true => stamped(image, x, y, inking.color, inking.opacity),
        false => turned(&image, x, y, inking),
    }
}

fn turned(image: &GlyphImage, x: i32, y: i32, inking: &Inking) -> Option<Stamp> {
    if image.width == 0 || image.height == 0 {
        return None;
    }
    let (width, height) = (image.width as usize, image.height as usize);
    let opacity = inking.opacity.clamp(0.0, 1.0);
    let premultiplied: Vec<[f32; 4]> = match image.kind {
        GlyphKind::Color => image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel.map(|channel| f32::from(channel) * opacity))
            .collect(),
        GlyphKind::Mask => {
            let color = inking.color;
            image
                .pixels
                .iter()
                .map(|&coverage| {
                    let alpha = f32::from(coverage) * color.a.clamp(0.0, 1.0) * opacity;
                    let [red, green, blue] =
                        [color.r, color.g, color.b].map(|channel| channel.clamp(0.0, 1.0) * alpha);
                    [red, green, blue, alpha]
                })
                .collect()
        }
    };
    let origin = Point::new((x + image.left) as f32, (y - image.top) as f32);
    let degrees = f64::from(inking.turn.degrees);
    let forward = Affine::about(inking.turn.centre, Affine::rotation(degrees));
    let back = Affine::about(inking.turn.centre, Affine::rotation(-degrees));
    let source = Rect::new(origin.x, origin.y, width as f32, height as f32);
    let reach = enclosing(source.corners().map(|corner| forward.apply(corner)));
    let (left, top) = (reach.min_x().floor() as i32, reach.min_y().floor() as i32);
    let across = (reach.max_x().ceil() as i32 - left).max(1) as u32;
    let down = (reach.max_y().ceil() as i32 - top).max(1) as u32;
    let texel = |column: i64, row: i64| -> [f32; 4] {
        match (0..width as i64).contains(&column) && (0..height as i64).contains(&row) {
            true => premultiplied[row as usize * width + column as usize],
            false => [0.0; 4],
        }
    };
    let mut pixels = Vec::with_capacity(across as usize * down as usize * 4);
    for row in 0..down {
        for column in 0..across {
            let shown = Point::new(
                left as f32 + column as f32 + 0.5,
                top as f32 + row as f32 + 0.5,
            );
            let at = back.apply(shown) - origin - Point::new(0.5, 0.5);
            let (floor_x, floor_y) = (at.x.floor(), at.y.floor());
            let (along_x, along_y) = (at.x - floor_x, at.y - floor_y);
            let (column, row) = (floor_x as i64, floor_y as i64);
            let corners = [
                (texel(column, row), (1.0 - along_x) * (1.0 - along_y)),
                (texel(column + 1, row), along_x * (1.0 - along_y)),
                (texel(column, row + 1), (1.0 - along_x) * along_y),
                (texel(column + 1, row + 1), along_x * along_y),
            ];
            let mixed: [f32; 4] = std::array::from_fn(|channel| {
                corners
                    .iter()
                    .map(|(texel, weight)| texel[channel] * weight)
                    .sum()
            });
            let alpha = mixed[3];
            let straight = |channel: f32| match alpha > 0.0 {
                true => (channel / alpha * 255.0).round().clamp(0.0, 255.0) as u8,
                false => 0,
            };
            pixels.extend([
                straight(mixed[0]),
                straight(mixed[1]),
                straight(mixed[2]),
                alpha.round().clamp(0.0, 255.0) as u8,
            ]);
        }
    }
    Stamp::new(across, down, pixels)
        .ok()
        .map(|stamp| stamp.with_position(left, top))
}

fn stamped(image: GlyphImage, x: i32, y: i32, color: Color, opacity: f32) -> Option<Stamp> {
    if image.width == 0 || image.height == 0 {
        return None;
    }
    let faded = |alpha: u8| match opacity >= 1.0 {
        true => alpha,
        false => (f32::from(alpha) * opacity.max(0.0)).round() as u8,
    };
    let pixels: Vec<u8> = match image.kind {
        GlyphKind::Color => image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[r, g, b, a]| {
                let straight = |channel: u8| match a {
                    0 => 0,
                    _ => ((u32::from(channel) * 255 + u32::from(a) / 2) / u32::from(a)).min(255)
                        as u8,
                };
                [straight(r), straight(g), straight(b), faded(a)]
            })
            .collect(),
        GlyphKind::Mask => {
            let ink = [color.r, color.g, color.b]
                .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
            image
                .pixels
                .iter()
                .flat_map(|&coverage| {
                    let alpha = (f32::from(coverage) * color.a.clamp(0.0, 1.0)).round() as u8;
                    [ink[0], ink[1], ink[2], faded(alpha)]
                })
                .collect()
        }
    };
    Stamp::new(image.width, image.height, pixels)
        .ok()
        .map(|stamp| stamp.with_position(x + image.left, y - image.top))
}
