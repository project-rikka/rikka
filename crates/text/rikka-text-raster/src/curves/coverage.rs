use rikka_text_font::Point;

use crate::curves::blob::{BandAxis, CurveBlob, CurveHeader};
use crate::curves::encoder::unpack;

const ROOTS_UP: u32 = 0x74;
const ROOTS_DOWN: u32 = 0x2E;
const MIN_WEIGHT: f32 = 1.0 / 65536.0;

#[derive(Clone, Copy, Debug)]
struct Ray {
    coverage: f32,
    weight: f32,
}

pub fn coverage_at(words: &[u32], x: f32, y: f32, pixels_per_em: f32) -> f32 {
    let Some(header) = CurveHeader::read(words) else {
        return 0.0;
    };
    let horizontal_bands = usize::from(header.horizontal_bands);
    let vertical_bands = usize::from(header.vertical_bands);
    let bounds = header.bounds;
    let pixel = Point::new(x, y);
    let horizontal = cast(
        words,
        &header,
        CurveBlob::BAND_HEADERS_WORD + band(y, bounds.y_min, bounds.y_max, horizontal_bands),
        pixel,
        pixels_per_em,
        BandAxis::Horizontal,
    );
    let vertical = cast(
        words,
        &header,
        CurveBlob::BAND_HEADERS_WORD
            + horizontal_bands
            + band(x, bounds.x_min, bounds.x_max, vertical_bands),
        pixel,
        pixels_per_em,
        BandAxis::Vertical,
    );
    let total = horizontal.weight + vertical.weight;
    let blended = (horizontal.coverage * horizontal.weight + vertical.coverage * vertical.weight)
        .abs()
        / total.max(MIN_WEIGHT);
    let inside = horizontal.coverage.abs().min(vertical.coverage.abs());
    blended.max(inside).clamp(0.0, 1.0)
}

fn band(value: f32, min: f32, max: f32, count: usize) -> usize {
    let position = ((value - min) / (max - min) * count as f32).floor();
    position.clamp(0.0, (count - 1) as f32) as usize
}

fn cast(
    words: &[u32],
    header: &CurveHeader,
    band_word: usize,
    pixel: Point,
    pixels_per_em: f32,
    axis: BandAxis,
) -> Ray {
    let vertical = axis == BandAxis::Vertical;
    let mut ray = Ray {
        coverage: 0.0,
        weight: 0.0,
    };
    let Some(&band) = words.get(band_word) else {
        return ray;
    };
    let offset = (band & CurveBlob::BAND_OFFSET_MASK) as usize;
    let count = (band >> CurveBlob::BAND_COUNT_SHIFT) as usize;
    for index in 0..count {
        let Some(&references) = words.get(offset + index / CurveBlob::REFERENCES_PER_WORD) else {
            break;
        };
        let curve =
            ((references >> (16 * (index % CurveBlob::REFERENCES_PER_WORD))) & 0xFFFF) as usize;
        let Some(points) = words.get(curve..curve + CurveBlob::CURVE_WORDS) else {
            break;
        };
        let relative = |word: u32| -> (f32, f32) {
            let point = unpack(word, &header.bounds);
            let along = (point.x - pixel.x) * pixels_per_em;
            let across = (point.y - pixel.y) * pixels_per_em;
            if vertical {
                (across, along)
            } else {
                (along, across)
            }
        };
        let (a0, c0) = relative(points[0]);
        let (a1, c1) = relative(points[1]);
        let (a2, c2) = relative(points[2]);
        if a0.max(a1).max(a2) < -0.5 {
            break;
        }
        let code = u32::from(c0 > 0.0) | u32::from(c1 > 0.0) << 1 | u32::from(c2 > 0.0) << 2;
        let counts_up = (ROOTS_UP >> code) & 1 == 1;
        let counts_down = (ROOTS_DOWN >> code) & 1 == 1;
        if !counts_up && !counts_down {
            continue;
        }
        let (up, down) = roots(c0, c1, c2);
        let quadratic = a0 - 2.0 * a1 + a2;
        let linear = a0 - a1;
        let along = |t: f32| (quadratic * t - 2.0 * linear) * t + a0;
        let sign = if vertical { -1.0 } else { 1.0 };
        for (counted, t, direction) in [(counts_up, up, sign), (counts_down, down, -sign)] {
            if counted {
                let distance = along(t);
                ray.coverage += direction * (distance + 0.5).clamp(0.0, 1.0);
                ray.weight = ray.weight.max((1.0 - 2.0 * distance.abs()).clamp(0.0, 1.0));
            }
        }
    }
    ray
}

fn roots(c0: f32, c1: f32, c2: f32) -> (f32, f32) {
    let a = c0 - 2.0 * c1 + c2;
    let b = c0 - c1;
    let root = (b * b - a * c0).max(0.0).sqrt();
    let (up, down) = if b >= 0.0 {
        let q = b + root;
        (divide(q, a), divide(c0, q))
    } else {
        let q = b - root;
        (divide(c0, q), divide(q, a))
    };
    (unit(up), unit(down))
}

fn unit(t: f32) -> f32 {
    if t >= 0.0 { t.min(1.0) } else { 0.0 }
}

fn divide(numerator: f32, denominator: f32) -> f32 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}
