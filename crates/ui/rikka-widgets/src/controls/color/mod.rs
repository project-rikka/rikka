mod comparison;
pub(crate) mod picker;
mod plane;
pub(crate) mod well;

use std::sync::Arc;

use rikka_types::{Color, Rect, RoundedRect};
use rikka_ui::{ImageSource, PaintCtx, Quad};

const CHECKER_CELLS: u32 = 64;
const CHECKER_CELL: f32 = 5.0;
const CHECKER_LIGHT: u8 = 0xff;
const CHECKER_DARK: u8 = 0xcc;
const RAMP: u32 = 256;
const HUES: u32 = 360;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hsv {
    pub(crate) hue: f32,
    pub(crate) saturation: f32,
    pub(crate) value: f32,
    pub(crate) alpha: f32,
}

impl Hsv {
    pub(crate) fn from_color(color: Color) -> Hsv {
        let [hue, saturation, value] = color.to_hsv();
        Hsv {
            hue,
            saturation,
            value,
            alpha: color.a.clamp(0.0, 1.0),
        }
    }

    pub(crate) fn color(self) -> Color {
        Color::from_hsv(self.hue, self.saturation, self.value).with_alpha(self.alpha)
    }

    pub(crate) fn pure_hue(self) -> Color {
        Hsv {
            saturation: 1.0,
            value: 1.0,
            alpha: 1.0,
            ..self
        }
        .color()
    }

    pub(crate) fn adopt(self, color: Color) -> Hsv {
        if same_color(self.color(), color) {
            return self;
        }
        let mut next = Hsv::from_color(color);
        if next.saturation == 0.0 || next.value == 0.0 {
            next.hue = self.hue;
        }
        if next.value == 0.0 {
            next.saturation = self.saturation;
        }
        next
    }
}

pub(crate) fn same_color(a: Color, b: Color) -> bool {
    const NOISE: f32 = 1e-5;
    (a.r - b.r).abs() < NOISE
        && (a.g - b.g).abs() < NOISE
        && (a.b - b.b).abs() < NOISE
        && (a.a - b.a).abs() < NOISE
}

pub(crate) fn hex_of(color: Color, alpha: bool) -> String {
    let shown = match alpha {
        true => color,
        false => color.with_alpha(1.0),
    };
    shown.to_hex().to_ascii_uppercase()
}

pub(crate) fn parse_hex(typed: &str, alpha: bool, kept_alpha: f32) -> Option<Color> {
    let typed = typed.trim();
    let digits = typed.strip_prefix('#').unwrap_or(typed);
    if !matches!(digits.len(), 3 | 6 | 8) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let parsed = Color::parse_hex(&format!("#{digits}"))?;
    let carries_alpha = alpha && digits.len() == 8;
    Some(match carries_alpha {
        true => parsed,
        false => parsed.with_alpha(kept_alpha),
    })
}

pub(crate) fn paint_checker(ctx: &mut PaintCtx<'_>, shape: RoundedRect) {
    let tile = CHECKER_CELL * CHECKER_CELLS as f32;
    let rect = shape.rect;
    let checker = textures().checker;
    ctx.clipped(shape, |ctx| {
        let mut y = rect.y();
        while y < rect.max_y() {
            let mut x = rect.x();
            while x < rect.max_x() {
                ctx.draw_image_pixelated(Rect::new(x, y, tile, tile), checker.clone());
                x += tile;
            }
            y += tile;
        }
    });
}

pub(crate) fn paint_swatch(ctx: &mut PaintCtx<'_>, shape: RoundedRect, color: Color) {
    if color.a < 1.0 {
        paint_checker(ctx, shape);
    }
    ctx.draw_quad(Quad::new(shape.rect).fill(color).radius(shape.radii));
}

#[derive(Clone)]
pub(crate) struct Textures {
    pub(crate) checker: ImageSource,
    pub(crate) fading_right: ImageSource,
    pub(crate) rising_right: ImageSource,
    pub(crate) rising_down: ImageSource,
    pub(crate) hues: ImageSource,
}

thread_local! {
    static TEXTURES: Textures = Textures::new();
}

pub(crate) fn textures() -> Textures {
    TEXTURES.with(Textures::clone)
}

impl Textures {
    fn new() -> Self {
        Self {
            checker: checker(),
            fading_right: ramp(RAMP, 1, |at| 1.0 - at),
            rising_right: ramp(RAMP, 1, |at| at),
            rising_down: ramp(1, RAMP, |at| at),
            hues: hues(),
        }
    }
}

fn rgba(width: u32, height: u32, pixels: Vec<u8>) -> ImageSource {
    ImageSource::Rgba {
        width,
        height,
        pixels: Arc::from(pixels),
    }
}

fn checker() -> ImageSource {
    let pixels = (0..CHECKER_CELLS * CHECKER_CELLS)
        .flat_map(|at| {
            let (x, y) = (at % CHECKER_CELLS, at / CHECKER_CELLS);
            let shade = match (x + y) % 2 {
                0 => CHECKER_LIGHT,
                _ => CHECKER_DARK,
            };
            [shade, shade, shade, u8::MAX]
        })
        .collect();
    rgba(CHECKER_CELLS, CHECKER_CELLS, pixels)
}

fn ramp(width: u32, height: u32, alpha: impl Fn(f32) -> f32) -> ImageSource {
    let length = width.max(height);
    let pixels = (0..length)
        .flat_map(|at| {
            let shade = alpha(at as f32 / (length - 1) as f32);
            [u8::MAX, u8::MAX, u8::MAX, (shade * 255.0).round() as u8]
        })
        .collect();
    rgba(width, height, pixels)
}

fn hues() -> ImageSource {
    let pixels = (0..HUES)
        .flat_map(|at| {
            let hue = Hsv {
                hue: at as f32 / (HUES - 1) as f32,
                saturation: 1.0,
                value: 1.0,
                alpha: 1.0,
            };
            hue.color().to_rgba8()
        })
        .collect();
    rgba(HUES, 1, pixels)
}
