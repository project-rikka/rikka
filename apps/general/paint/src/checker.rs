use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use rikka_app::prelude::{Color, PaintCtx};
use rikka_image_edit::Checkerboard;
use rikka_types::{Point, Rect, RoundedRect};
use rikka_ui::ImageSource;

const SQUARES: u32 = 32;
const LIGHT: [u8; 4] = [255, 255, 255, 255];
const DARK: [u8; 4] = [204, 204, 204, 255];

thread_local! {
    static PATTERNS: RefCell<HashMap<u32, ImageSource>> = RefCell::new(HashMap::new());
}

pub(crate) fn fill(ctx: &mut PaintCtx<'_>, area: Rect, anchor: Point, square: f32) {
    let scale = ctx.scale();
    let side = (square * scale).round().max(1.0) as u32;
    let pattern = pattern(side);
    let tile = (side * SQUARES) as f32 / scale;
    let first = |edge: f32, anchor: f32| anchor + ((edge - anchor) / tile).floor() * tile;
    ctx.clipped(RoundedRect::new(area, 0.0), |ctx| {
        let mut y = first(area.min_y(), anchor.y);
        while y < area.max_y() {
            let mut x = first(area.min_x(), anchor.x);
            while x < area.max_x() {
                ctx.draw_image_pixelated(Rect::new(x, y, tile, tile), pattern.clone());
                x += tile;
            }
            y += tile;
        }
    });
}

pub(crate) fn checkerboard(square: f32, scale: f32, origin: Point) -> Checkerboard {
    let [r, g, b, a] = LIGHT;
    let [dr, dg, db, da] = DARK;
    Checkerboard {
        light: Color::rgba8(r, g, b, a),
        dark: Color::rgba8(dr, dg, db, da),
        square: (square * scale).round().max(1.0),
        origin,
    }
}

fn pattern(side: u32) -> ImageSource {
    PATTERNS.with(|patterns| {
        patterns
            .borrow_mut()
            .entry(side)
            .or_insert_with(|| drawn(side))
            .clone()
    })
}

fn drawn(side: u32) -> ImageSource {
    let width = side * SQUARES;
    let pixels: Vec<u8> = (0..width * width)
        .flat_map(|index| {
            let (x, y) = (index % width / side, index / width / side);
            match (x + y) % 2 == 0 {
                true => LIGHT,
                false => DARK,
            }
        })
        .collect();
    ImageSource::Rgba {
        width,
        height: width,
        pixels: Arc::from(pixels),
    }
}
