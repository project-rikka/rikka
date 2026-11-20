use crate::area::Area;
use crate::combine::Combine;
use crate::document::check_memory_budget;
use crate::edit::SelectionAction;
use crate::error::Result;
use crate::raster::{Raster, widened};
use crate::region::{Builder, Region, Span};
use crate::shape::quantized;

const MAX_RADIUS: u32 = 500;
const MAX_BORDER: u32 = 200;
const MAX_FEATHER: f32 = 1000.0;
const DIRECT_SIGMA: f32 = 4.0;
const BOX_PASSES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Modify {
    Expand(u32),
    Contract(u32),
    Feather(f32),
    Border(u32),
    Smooth(u32),
}

impl Modify {
    pub(crate) fn selecting(self) -> SelectionAction {
        match self {
            Modify::Expand(_) => SelectionAction::Expand,
            Modify::Contract(_) => SelectionAction::Contract,
            Modify::Feather(_) => SelectionAction::Feather,
            Modify::Border(_) => SelectionAction::Border,
            Modify::Smooth(_) => SelectionAction::Smooth,
        }
    }
}

impl Region {
    pub fn modified(&self, modify: Modify, canvas: Area) -> Result<Region> {
        let region = self.clipped(canvas);
        if region.is_empty() {
            return Ok(region);
        }
        Ok(match modify {
            Modify::Expand(radius) => region.dilated(radius.min(MAX_RADIUS), canvas),
            Modify::Contract(radius) => region.contracted(radius.min(MAX_RADIUS), canvas),
            Modify::Border(width) => {
                let width = width.clamp(1, MAX_BORDER);
                let outer = region.dilated(width.div_ceil(2), canvas);
                outer.combine(Combine::Exclude, region.contracted(width / 2, canvas))
            }
            Modify::Feather(radius) => region.feathered(radius, canvas)?,
            Modify::Smooth(radius) => region.smoothed(radius.min(MAX_RADIUS), canvas)?,
        })
    }

    fn contracted(&self, radius: u32, canvas: Area) -> Region {
        self.inverted(canvas)
            .dilated(radius, canvas)
            .inverted(canvas)
    }

    fn dilated(&self, radius: u32, canvas: Area) -> Region {
        if radius == 0 || self.is_empty() {
            return self.clone();
        }
        let reach: Vec<u32> = (0..=radius)
            .map(|dy| f64::from(radius * radius - dy * dy).sqrt().floor() as u32)
            .collect();
        let lines = self.lines();
        let top = lines.start.saturating_sub(radius).max(canvas.top);
        let bottom = (lines.end + radius).min(canvas.top + canvas.height);
        let mut builder = Builder::new(top);
        let mut pieces = Vec::new();
        let mut flattening = Flattening::default();
        for y in top..bottom {
            pieces.clear();
            let from = y.saturating_sub(radius).max(lines.start);
            let to = (y + radius + 1).min(lines.end);
            for source in from..to {
                let by = reach[source.abs_diff(y) as usize];
                pieces.extend(self.row(source).iter().map(|span| Span {
                    start: span.start.saturating_sub(by).max(canvas.left),
                    end: (span.end + by).min(canvas.right()),
                    coverage: span.coverage,
                }));
            }
            flattening.flatten(&mut pieces, &mut builder);
            builder.end_row();
        }
        builder.finish()
    }

    fn feathered(&self, radius: f32, canvas: Area) -> Result<Region> {
        let sigma = radius.clamp(0.0, MAX_FEATHER) / 2.0;
        let Some(extent) = self.extent() else {
            return Ok(Region::default());
        };
        if sigma < 0.25 {
            return Ok(self.clone());
        }
        let area = widened(extent, (sigma * 3.0).ceil() as u32 + 1, canvas);
        let mut raster = Raster::rasterize(self, area)?;
        let (width, height) = (area.width as usize, area.height as usize);
        let mut line = Vec::new();
        let mut scratch = Vec::new();
        for row in raster.values.chunks_mut(width) {
            line.clear();
            line.extend(row.iter().map(|value| f32::from(*value) / 255.0));
            blur(&mut line, &mut scratch, sigma);
            for (value, blurred) in row.iter_mut().zip(&line) {
                *value = quantized(*blurred);
            }
        }
        for column in 0..width {
            line.clear();
            line.extend(
                (0..height).map(|row| f32::from(raster.values[row * width + column]) / 255.0),
            );
            blur(&mut line, &mut scratch, sigma);
            for (row, blurred) in line.iter().enumerate() {
                raster.values[row * width + column] = quantized(*blurred);
            }
        }
        Ok(raster.region())
    }

    fn smoothed(&self, radius: u32, canvas: Area) -> Result<Region> {
        let footprint = self.footprint();
        let Some(bounds) = footprint.bounds() else {
            return Ok(Region::default());
        };
        if radius == 0 {
            return Ok(footprint);
        }
        let area = widened(bounds, radius, canvas);
        let raster = Raster::rasterize(&footprint, area)?;
        let (width, height) = (area.width as usize, area.height as usize);
        check_memory_budget(area.width, area.height, (width * height * 2) as u64)?;
        let reach = radius as usize;
        let mut across = vec![0u16; width * height];
        for (row, counts) in across.chunks_mut(width).enumerate() {
            let line = raster.row(row as u32);
            let mut covered = line[..reach.min(width)]
                .iter()
                .filter(|value| **value > 0)
                .count() as u16;
            for (x, count) in counts.iter_mut().enumerate() {
                if x + reach < width && line[x + reach] > 0 {
                    covered += 1;
                }
                *count = covered;
                if x >= reach && line[x - reach] > 0 {
                    covered -= 1;
                }
            }
        }
        let span = |at: usize, low: u32, high: u32, origin: u32| {
            let at = i64::from(origin) + at as i64;
            let from = (at - reach as i64).max(i64::from(low));
            let to = (at + reach as i64).min(i64::from(high) - 1);
            (to - from + 1) as u64
        };
        let mut down = vec![0u32; width];
        for row in 0..reach.min(height) {
            for (sum, count) in down.iter_mut().zip(&across[row * width..(row + 1) * width]) {
                *sum += u32::from(*count);
            }
        }
        let mut builder = Builder::new(area.top);
        let bottom = canvas.top + canvas.height;
        for row in 0..height {
            if row + reach < height {
                let entering = &across[(row + reach) * width..(row + reach + 1) * width];
                for (sum, count) in down.iter_mut().zip(entering) {
                    *sum += u32::from(*count);
                }
            }
            let tall = span(row, canvas.top, bottom, area.top);
            let line = raster.row(row as u32);
            let mut run: Option<u32> = None;
            for (x, sum) in down.iter().enumerate() {
                let wide = span(x, canvas.left, canvas.right(), area.left);
                let twice = u64::from(*sum) * 2;
                let room = wide * tall;
                let survives = twice > room || (twice == room && line[x] > 0);
                let x = area.left + x as u32;
                match (survives, run) {
                    (true, None) => run = Some(x),
                    (false, Some(start)) => {
                        builder.push(start, x, Span::FULL);
                        run = None;
                    }
                    _ => {}
                }
            }
            if let Some(start) = run {
                builder.push(start, area.right(), Span::FULL);
            }
            builder.end_row();
            if row >= reach {
                let leaving = &across[(row - reach) * width..(row - reach + 1) * width];
                for (sum, count) in down.iter_mut().zip(leaving) {
                    *sum -= u32::from(*count);
                }
            }
        }
        Ok(builder.finish())
    }
}

#[derive(Default)]
struct Flattening {
    ends: Vec<(u32, u8)>,
}

impl Flattening {
    fn flatten(&mut self, pieces: &mut [Span], builder: &mut Builder) {
        pieces.sort_unstable_by_key(|piece| piece.start);
        self.ends.clear();
        self.ends.extend(
            pieces
                .iter()
                .filter(|piece| piece.start < piece.end)
                .map(|piece| (piece.end, piece.coverage)),
        );
        self.ends.sort_unstable();
        let mut counts = [0u32; 256];
        let mut level = 0u8;
        let (mut starts, mut closed) = (0, 0);
        let Some(first) = pieces.iter().find(|piece| piece.start < piece.end) else {
            return;
        };
        let mut x = first.start;
        loop {
            while closed < self.ends.len() && self.ends[closed].0 <= x {
                let coverage = self.ends[closed].1;
                counts[usize::from(coverage)] -= 1;
                if coverage == level && counts[usize::from(coverage)] == 0 {
                    level = (0..coverage)
                        .rev()
                        .find(|lower| counts[usize::from(*lower)] > 0)
                        .unwrap_or(0);
                }
                closed += 1;
            }
            while starts < pieces.len() && pieces[starts].start <= x {
                let piece = pieces[starts];
                if piece.start < piece.end {
                    counts[usize::from(piece.coverage)] += 1;
                    level = level.max(piece.coverage);
                }
                starts += 1;
            }
            let next = match (pieces.get(starts), self.ends.get(closed)) {
                (None, None) => break,
                (Some(piece), None) => piece.start,
                (None, Some(end)) => end.0,
                (Some(piece), Some(end)) => piece.start.min(end.0),
            };
            builder.push(x, next, level);
            x = next;
        }
    }
}

fn blur(line: &mut Vec<f32>, scratch: &mut Vec<f32>, sigma: f32) {
    scratch.clear();
    scratch.resize(line.len(), 0.0);
    match sigma <= DIRECT_SIGMA {
        true => {
            let reach = (sigma * 3.0).ceil() as i64;
            let weights: Vec<f32> = (-reach..=reach)
                .map(|offset| (-((offset * offset) as f32) / (2.0 * sigma * sigma)).exp())
                .collect();
            let total: f32 = weights.iter().sum();
            let last = line.len() as i64 - 1;
            for (x, out) in scratch.iter_mut().enumerate() {
                *out = weights
                    .iter()
                    .enumerate()
                    .map(|(at, weight)| {
                        let source = (x as i64 + at as i64 - reach).clamp(0, last);
                        weight * line[source as usize]
                    })
                    .sum::<f32>()
                    / total;
            }
            std::mem::swap(line, scratch);
        }
        false => {
            let ideal = (12.0 * sigma * sigma / BOX_PASSES as f32 + 1.0).sqrt();
            let reach = ((ideal - 1.0) / 2.0).round().max(1.0) as usize;
            for _ in 0..BOX_PASSES {
                box_pass(line, scratch, reach);
                std::mem::swap(line, scratch);
            }
        }
    }
}

fn box_pass(source: &[f32], out: &mut [f32], reach: usize) {
    let last = source.len() as i64 - 1;
    let at = |index: i64| source[index.clamp(0, last) as usize];
    let reach = reach as i64;
    let mut sum: f32 = (-reach..=reach).map(at).sum();
    let width = (2 * reach + 1) as f32;
    for (x, value) in out.iter_mut().enumerate() {
        *value = sum / width;
        let x = x as i64;
        sum += at(x + reach + 1) - at(x - reach);
    }
}
