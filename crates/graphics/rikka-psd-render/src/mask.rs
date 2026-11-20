use std::borrow::Borrow;
use std::ops::Range;

use rikka_psd::Rect;

use crate::table::sample;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mask {
    pub(crate) bounds: Rect,
    pub(crate) data: Vec<f32>,
    pub(crate) outside: f32,
}

impl Mask {
    pub(crate) fn new(bounds: Rect, value: f32) -> Self {
        Self {
            bounds,
            data: vec![value; bounds.area() as usize],
            outside: 0.0,
        }
    }

    pub(crate) fn filled(bounds: Rect, value: f32, outside: f32) -> Self {
        Self {
            bounds,
            data: vec![value; bounds.area() as usize],
            outside,
        }
    }

    pub(crate) fn width(&self) -> usize {
        self.bounds.width() as usize
    }

    pub(crate) fn height(&self) -> usize {
        self.bounds.height() as usize
    }

    pub(crate) fn get(&self, x: i32, y: i32) -> f32 {
        if !self.bounds.contains(x, y) {
            return self.outside;
        }
        self.data[(y - self.bounds.top) as usize * self.width() + (x - self.bounds.left) as usize]
    }

    pub(crate) fn resized(&self, bounds: Rect) -> Mask {
        let mut out = Mask::filled(bounds, self.outside, self.outside);
        let width = out.width();
        if width > 0 {
            for (y, row) in (bounds.top..).zip(out.data.chunks_exact_mut(width)) {
                self.row_into(y, bounds.left, row);
            }
        }
        out
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, value: f32) {
        if self.bounds.contains(x, y) {
            let width = self.width();
            self.data[(y - self.bounds.top) as usize * width + (x - self.bounds.left) as usize] =
                value;
        }
    }

    pub(crate) fn map(&mut self, function: impl Fn(f32) -> f32) {
        self.data
            .iter_mut()
            .for_each(|value| *value = function(*value));
        self.outside = function(self.outside);
    }

    pub(crate) fn multiply(&mut self, other: &Mask) {
        let (left, width) = (self.bounds.left, self.width());
        if width > 0 {
            for (y, row) in (self.bounds.top..).zip(self.data.chunks_exact_mut(width)) {
                other.scale_row(y, left, row);
            }
        }
        self.outside *= other.outside;
    }

    pub(crate) fn row_into(&self, y: i32, left: i32, out: &mut [f32]) {
        let (inside, values) = self.covered(y, left, out.len());
        out[..inside.start].fill(self.outside);
        out[inside.end..].fill(self.outside);
        out[inside].copy_from_slice(values);
    }

    fn scale_row(&self, y: i32, left: i32, out: &mut [f32]) {
        let (inside, values) = self.covered(y, left, out.len());
        let (before, rest) = out.split_at_mut(inside.start);
        let (covered, after) = rest.split_at_mut(inside.len());
        for value in before.iter_mut().chain(after) {
            *value *= self.outside;
        }
        for (value, factor) in covered.iter_mut().zip(values) {
            *value *= factor;
        }
    }

    fn covered(&self, y: i32, left: i32, len: usize) -> (Range<usize>, &[f32]) {
        let clip = |x: i32| (x - left).clamp(0, len as i32) as usize;
        let inside = clip(self.bounds.left)..clip(self.bounds.right);
        if inside.is_empty() || y < self.bounds.top || y >= self.bounds.bottom {
            return (0..0, &[]);
        }
        let start = (y - self.bounds.top) as usize * self.width()
            + (left + inside.start as i32 - self.bounds.left) as usize;
        (inside.clone(), &self.data[start..start + inside.len()])
    }

    pub(crate) fn inverted(&self) -> Mask {
        let mut out = self.clone();
        out.map(|value| 1.0 - value);
        out
    }

    pub(crate) fn shifted(&self, dx: f32, dy: f32) -> Mask {
        let mut out = Mask::filled(self.bounds, 0.0, self.outside);
        let width = self.width();
        if width == 0 {
            return out;
        }
        let (column, fx) = whole_and_fraction(-dx);
        let (row, fy) = whole_and_fraction(-dy);
        let mut upper = vec![0.0f32; width + 1];
        let mut lower = vec![0.0f32; width + 1];
        let left = self.bounds.left + column;
        for (y, target) in (self.bounds.top..).zip(out.data.chunks_exact_mut(width)) {
            self.row_into(y + row, left, &mut upper);
            self.row_into(y + row + 1, left, &mut lower);
            for (slot, ((top, bottom), (top_next, bottom_next))) in target.iter_mut().zip(
                upper
                    .iter()
                    .zip(&lower)
                    .zip(upper[1..].iter().zip(&lower[1..])),
            ) {
                let high = top + (top_next - top) * fx;
                let low = bottom + (bottom_next - bottom) * fx;
                *slot = high + (low - high) * fy;
            }
        }
        out
    }

    pub(crate) fn blurred(&self, sigma: f32) -> Mask {
        if sigma < 0.3 {
            return self.clone();
        }
        let mut out = self.clone();
        let (width, height) = (self.width(), self.height());
        let mut scratch = vec![0.0f32; width];
        let mut spare = vec![0.0f32; out.data.len()];
        for radius in box_sizes(sigma) {
            if radius == 0 || width == 0 {
                continue;
            }
            for line in out.data.chunks_exact_mut(width) {
                box_pass(line, &mut scratch, radius, self.outside);
            }
            slide(&out.data, &mut spare, width, height, radius, self.outside);
            std::mem::swap(&mut out.data, &mut spare);
        }
        out
    }

    pub(crate) fn distance_outside(&self) -> Vec<f32> {
        distance(self, |value| value >= 0.5).0
    }

    pub(crate) fn distance_inside(&self) -> Vec<f32> {
        distance(self, |value| value < 0.5).0
    }

    pub(crate) fn signed_distance(&self) -> Vec<f32> {
        let outside = self.distance_outside();
        let inside = self.distance_inside();
        let base: Vec<f32> = self
            .data
            .iter()
            .zip(outside.iter().zip(&inside))
            .map(|(&coverage, (&out, &inn))| {
                if coverage >= 0.5 {
                    0.5 - inn
                } else {
                    out - 0.5
                }
            })
            .collect();
        let partial = |value: f32| value > 1e-3 && value < 1.0 - 1e-3;
        if !self.data.iter().any(|&value| partial(value)) {
            return base;
        }
        let (to_partial, nearest) = distance(self, partial);
        self.data
            .iter()
            .enumerate()
            .map(|(index, &coverage)| {
                if partial(coverage) {
                    return 0.5 - coverage;
                }
                let stated = 0.5 - self.data[nearest[index] as usize];
                if coverage >= 0.5 {
                    (stated - to_partial[index]).max(base[index])
                } else {
                    (to_partial[index] + stated).min(base[index])
                }
            })
            .collect()
    }

    pub(crate) fn dilated(&self, radius: f32) -> Mask {
        if radius <= 0.0 {
            return self.clone();
        }
        let outside = self.distance_outside();
        let mut out = self.clone();
        for (slot, &distance) in out.data.iter_mut().zip(&outside) {
            let reach = (radius - (distance - 0.5)).clamp(0.0, 1.0);
            *slot = slot.max(reach);
        }
        out
    }

    pub(crate) fn with_lut(&mut self, lut: &[f32; 256]) {
        self.map(|value| sample(lut, value));
    }
}

pub(crate) fn coverage_row(
    amount: f32,
    masks: &[impl Borrow<Mask>],
    y: i32,
    left: i32,
    out: &mut [f32],
) {
    out.fill(amount);
    for mask in masks {
        mask.borrow().scale_row(y, left, out);
    }
}

fn whole_and_fraction(offset: f32) -> (i32, f32) {
    let whole = offset.floor();
    (whole as i32, offset - whole)
}

fn box_sizes(sigma: f32) -> [usize; 3] {
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut lower = ideal.floor() as usize;
    if lower.is_multiple_of(2) {
        lower = lower.saturating_sub(1).max(1);
    }
    let upper = lower + 2;
    let m = ((12.0 * sigma * sigma - 3.0 * (lower * lower) as f32 - 12.0 * lower as f32 - 9.0)
        / (-4.0 * lower as f32 - 4.0))
        .round();
    let mut sizes = [0usize; 3];
    for (index, size) in sizes.iter_mut().enumerate() {
        *size = if (index as f32) < m { lower } else { upper } / 2;
    }
    sizes
}

fn box_pass(line: &mut [f32], scratch: &mut [f32], radius: usize, outside: f32) {
    if radius == 0 || line.is_empty() {
        return;
    }
    let length = line.len();
    let width = (2 * radius + 1) as f32;
    let at = |index: isize| -> f32 {
        if index < 0 || index as usize >= length {
            outside
        } else {
            line[index as usize]
        }
    };
    let mut sum: f32 = (-(radius as isize)..=radius as isize).map(at).sum();
    for (index, slot) in scratch.iter_mut().enumerate().take(length) {
        *slot = sum / width;
        sum += at(index as isize + radius as isize + 1) - at(index as isize - radius as isize);
    }
    line.copy_from_slice(&scratch[..length]);
}

fn slide(
    source: &[f32],
    target: &mut [f32],
    width: usize,
    height: usize,
    radius: usize,
    outside: f32,
) {
    let beyond = vec![outside; width];
    let line = |index: isize| match usize::try_from(index) {
        Ok(row) if row < height => &source[row * width..(row + 1) * width],
        _ => &beyond,
    };
    let reach = radius as isize;
    let mut sum = vec![0.0f32; width];
    for index in -reach..=reach {
        for (total, value) in sum.iter_mut().zip(line(index)) {
            *total += value;
        }
    }
    let size = (2 * radius + 1) as f32;
    for (row, out) in (0..).zip(target.chunks_exact_mut(width)) {
        let (entering, leaving) = (line(row + reach + 1), line(row - reach));
        for (((slot, total), entering), leaving) in
            out.iter_mut().zip(&mut sum).zip(entering).zip(leaving)
        {
            *slot = *total / size;
            *total += entering - leaving;
        }
    }
}

fn distance(mask: &Mask, inside: impl Fn(f32) -> bool) -> (Vec<f32>, Vec<u32>) {
    let (width, height) = (mask.width(), mask.height());
    const FAR: f32 = 1e10;
    let mut grid: Vec<f32> = mask
        .data
        .iter()
        .map(|&value| if inside(value) { 0.0 } else { FAR })
        .collect();
    let mut rows = vec![0u32; width * height];
    let longest = width.max(height);
    let mut f = vec![0.0f32; longest];
    let mut d = vec![0.0f32; longest];
    let mut site = vec![0usize; longest];
    let mut v = vec![0usize; longest];
    let mut z = vec![0.0f32; longest + 1];
    for x in 0..width {
        for y in 0..height {
            f[y] = grid[y * width + x];
        }
        transform(
            &f[..height],
            &mut d[..height],
            &mut site[..height],
            &mut v,
            &mut z,
        );
        for y in 0..height {
            grid[y * width + x] = d[y];
            rows[y * width + x] = site[y] as u32;
        }
    }
    let mut nearest = vec![0u32; width * height];
    for y in 0..height {
        f[..width].copy_from_slice(&grid[y * width..(y + 1) * width]);
        transform(
            &f[..width],
            &mut d[..width],
            &mut site[..width],
            &mut v,
            &mut z,
        );
        grid[y * width..(y + 1) * width].copy_from_slice(&d[..width]);
        for x in 0..width {
            let column = site[x];
            nearest[y * width + x] = rows[y * width + column] * width as u32 + column as u32;
        }
    }
    (grid.iter().map(|value| value.sqrt()).collect(), nearest)
}

fn transform(f: &[f32], d: &mut [f32], site: &mut [usize], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q as f32 - p as f32));
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break;
            }
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        let delta = q as f32 - p as f32;
        d[q] = delta * delta + f[p];
        site[q] = p;
    }
}
