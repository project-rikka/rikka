use rikka_gpu::ComputedFrame;

use crate::area::Area;
use crate::error::{Error, Result};

const DEEPEST_LEVEL: u32 = 12;

#[derive(Clone, Debug)]
pub struct Tile {
    pub area: Area,
    pub frame: ComputedFrame,
}

impl Tile {
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.frame.width() as f32 / self.area.width.max(1) as f32
    }
}

pub(crate) fn cut(
    computed: &ComputedFrame,
    width: u32,
    height: u32,
    side: u32,
) -> Result<Vec<Tile>> {
    let divisor = divisor(width, height, (computed.width(), computed.height()));
    areas(computed.width(), computed.height(), side)
        .into_iter()
        .map(|piece| {
            let frame = computed
                .area([piece.left, piece.top], [piece.width, piece.height])
                .map_err(Error::from)?;
            let left = piece.left * divisor;
            let top = piece.top * divisor;
            let right = ((piece.left + piece.width) * divisor).min(width);
            let bottom = ((piece.top + piece.height) * divisor).min(height);
            Ok(Tile {
                area: Area::new(left, top, right - left, bottom - top),
                frame,
            })
        })
        .collect()
}

#[must_use]
pub fn level_for(scale: f32) -> u32 {
    if scale.is_nan() || scale <= 0.0 || scale >= 1.0 {
        return 0;
    }
    (1.0 / scale)
        .log2()
        .floor()
        .clamp(0.0, DEEPEST_LEVEL as f32) as u32
}

pub(crate) fn reduced(width: u32, height: u32, level: u32) -> (u32, u32) {
    let divisor = 1u32 << level;
    (
        width.div_ceil(divisor).max(1),
        height.div_ceil(divisor).max(1),
    )
}

fn divisor(width: u32, height: u32, frame_size: (u32, u32)) -> u32 {
    (0..=DEEPEST_LEVEL)
        .find(|level| reduced(width, height, *level) == frame_size)
        .map_or(1, |level| 1 << level)
}

pub(crate) fn areas(width: u32, height: u32, side: u32) -> Vec<Area> {
    let side = side.max(1);
    let mut areas = Vec::new();
    for y in (0..height).step_by(side as usize) {
        for x in (0..width).step_by(side as usize) {
            areas.push(Area::new(x, y, side.min(width - x), side.min(height - y)));
        }
    }
    areas
}
