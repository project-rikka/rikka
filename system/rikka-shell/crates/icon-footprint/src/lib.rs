mod hull;

use std::f32::consts::FRAC_PI_4;

use hull::Corner;

const VISIBLE_ALPHA: u8 = 40;

const CIRCLE_SHARE: f32 = 380.0 / 576.0;

const SQUARE_SHARE: f32 = 375.0 / 576.0;

const SHARE_PER_ROUNDNESS: f32 = (CIRCLE_SHARE - SQUARE_SHARE) / (1.0 - FRAC_PI_4);

#[must_use]
pub fn scale(width: u32, height: u32, rgba: &[u8]) -> Option<f32> {
    let (columns, rows) = (width as usize, height as usize);
    if columns == 0 || rgba.len() != columns.checked_mul(rows)?.checked_mul(4)? {
        return None;
    }

    let visible = |pixel: &[u8; 4]| pixel[3] > VISIBLE_ALPHA;
    let mut corners: Vec<Corner> = Vec::new();
    let (mut left, mut right) = (i64::MAX, i64::MIN);
    let (mut top, mut bottom) = (i64::MAX, i64::MIN);
    for (y, row) in rgba.chunks_exact(columns * 4).enumerate() {
        let (pixels, _) = row.as_chunks::<4>();
        let (Some(first), Some(last)) = (
            pixels.iter().position(visible),
            pixels.iter().rposition(visible),
        ) else {
            continue;
        };
        let (first, last) = (first as i64, last as i64 + 1);
        let (y, below) = (y as i64, y as i64 + 1);
        corners.extend([(first, y), (last, y), (first, below), (last, below)]);
        left = left.min(first);
        right = right.max(last);
        top = top.min(y);
        bottom = bottom.max(below);
    }
    if corners.is_empty() {
        return None;
    }

    let covered = hull::area(corners);
    let roundness = covered / ((right - left) * (bottom - top)) as f32;
    let allowed = match roundness < FRAC_PI_4 {
        true => CIRCLE_SHARE,
        false => SQUARE_SHARE + SHARE_PER_ROUNDNESS * (1.0 - roundness),
    };
    let share = covered / (columns * rows) as f32;
    Some(match share > allowed {
        true => (allowed / share).sqrt(),
        false => 1.0,
    })
}
