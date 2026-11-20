use std::cell::RefCell;

use rikka_text::{GlyphImage, GlyphKind, GlyphRaster, TextLayout};
use rikka_types::{Color, Rect};

const SOLID: u8 = 128;

thread_local! {
    static GLYPHS: RefCell<GlyphRaster> = RefCell::new(GlyphRaster::new());
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

struct PlacedGlyph {
    left: i32,
    top: i32,
    image: GlyphImage,
}

impl PlacedGlyph {
    fn right(&self) -> i32 {
        self.left + self.image.width as i32
    }

    fn bottom(&self) -> i32 {
        self.top + self.image.height as i32
    }
}

pub(crate) fn with_glyphs<R>(work: impl FnOnce(&mut GlyphRaster) -> R) -> R {
    GLYPHS.with(|glyphs| work(&mut glyphs.borrow_mut()))
}

fn placed(laid: &TextLayout, origin: (i32, i32)) -> Vec<PlacedGlyph> {
    let (x, y) = (origin.0 as f32, origin.1 as f32);
    with_glyphs(|glyphs| {
        let mut placed = Vec::new();
        for row in laid.rows() {
            for glyph in row.placed((x, y + row.baseline()), 1.0) {
                let Some(image) = glyphs.raster(glyph.key) else {
                    continue;
                };
                if image.width > 0 && image.height > 0 {
                    placed.push(PlacedGlyph {
                        left: glyph.x + image.left,
                        top: glyph.y - image.top,
                        image,
                    });
                }
            }
        }
        placed
    })
}

pub(crate) fn bounds(laid: &TextLayout, origin: (i32, i32)) -> Option<Rect> {
    let glyphs = placed(laid, origin);
    let left = glyphs.iter().map(|glyph| glyph.left).min()?;
    let top = glyphs.iter().map(|glyph| glyph.top).min()?;
    let right = glyphs.iter().map(PlacedGlyph::right).max()?;
    let bottom = glyphs.iter().map(PlacedGlyph::bottom).max()?;
    Some(Rect::new(
        left as f32,
        top as f32,
        (right - left) as f32,
        (bottom - top) as f32,
    ))
}

pub(crate) fn raster(
    laid: &TextLayout,
    origin: (i32, i32),
    within: Rect,
    color: Color,
    antialias: bool,
) -> Option<Raster> {
    let glyphs = placed(laid, origin);
    let limit = (
        within.min_x().floor() as i64,
        within.min_y().floor() as i64,
        within.max_x().ceil() as i64,
        within.max_y().ceil() as i64,
    );
    let left = glyphs.iter().map(|glyph| glyph.left).min()?;
    let top = glyphs.iter().map(|glyph| glyph.top).min()?;
    let right = glyphs.iter().map(PlacedGlyph::right).max()?;
    let bottom = glyphs.iter().map(PlacedGlyph::bottom).max()?;
    let left = i64::from(left).max(limit.0);
    let top = i64::from(top).max(limit.1);
    let right = i64::from(right).min(limit.2);
    let bottom = i64::from(bottom).min(limit.3);
    if left >= right || top >= bottom {
        return None;
    }
    let (width, height) = ((right - left) as usize, (bottom - top) as usize);
    let ink = [color.r, color.g, color.b].map(|channel| channel.clamp(0.0, 1.0));
    let mut canvas = Canvas {
        left,
        top,
        width,
        height,
        ink,
        premultiplied: vec![[0.0; 4]; width * height],
    };
    for glyph in &glyphs {
        canvas.lay(glyph, antialias);
    }
    Some(Raster {
        left: left as i32,
        top: top as i32,
        width: width as u32,
        height: height as u32,
        pixels: canvas.straight(color.a.clamp(0.0, 1.0)),
    })
}

struct Canvas {
    left: i64,
    top: i64,
    width: usize,
    height: usize,
    ink: [f32; 3],
    premultiplied: Vec<[f32; 4]>,
}

impl Canvas {
    fn lay(&mut self, glyph: &PlacedGlyph, antialias: bool) {
        let image = &glyph.image;
        let bytes = image.kind.bytes_per_pixel();
        for row in 0..image.height as usize {
            let y = i64::from(glyph.top) + row as i64 - self.top;
            if y < 0 || y >= self.height as i64 {
                continue;
            }
            for column in 0..image.width as usize {
                let x = i64::from(glyph.left) + column as i64 - self.left;
                if x < 0 || x >= self.width as i64 {
                    continue;
                }
                let at = (row * image.width as usize + column) * bytes;
                let source = match image.kind {
                    GlyphKind::Mask => {
                        let coverage = coverage(image.pixels[at], antialias);
                        let [r, g, b] = self.ink.map(|channel| channel * coverage);
                        [r, g, b, coverage]
                    }
                    GlyphKind::Color => {
                        [0, 1, 2, 3].map(|channel| f32::from(image.pixels[at + channel]) / 255.0)
                    }
                };
                let destination = &mut self.premultiplied[y as usize * self.width + x as usize];
                let keep = 1.0 - source[3];
                for (destination, laid) in destination.iter_mut().zip(source) {
                    *destination = laid + *destination * keep;
                }
            }
        }
    }

    fn straight(&self, opacity: f32) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.premultiplied.len() * 4);
        for destination in &self.premultiplied {
            let alpha = destination[3].clamp(0.0, 1.0);
            let rgb = match alpha > 0.0 {
                true => [destination[0], destination[1], destination[2]]
                    .map(|channel| (channel / alpha).clamp(0.0, 1.0)),
                false => self.ink,
            };
            pixels.extend(rgb.map(to_byte));
            pixels.push(to_byte(alpha * opacity));
        }
        pixels
    }
}

fn coverage(value: u8, antialias: bool) -> f32 {
    match (antialias, value >= SOLID) {
        (true, _) => f32::from(value) / 255.0,
        (false, true) => 1.0,
        (false, false) => 0.0,
    }
}

fn to_byte(value: f32) -> u8 {
    (value * 255.0).round() as u8
}
