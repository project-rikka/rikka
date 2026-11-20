use crate::area::Area;
use crate::error::{Error, Result};
use crate::layer::LayerId;
use crate::raster::{Raster, widened};
use crate::region::{Builder, Region, Span};

const DEFAULT_TOLERANCE: f32 = 0.5;
const WEIGHTS: [[u32; 3]; 3] = [[1, 2, 1], [2, 4, 2], [1, 2, 1]];
const WEIGHT: u32 = 16;
const HALF: u8 = 128;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FloodMode {
    #[default]
    Contiguous,
    Global,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Source {
    #[default]
    Image,
    Layer(LayerId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flood {
    x: f32,
    y: f32,
    tolerance: f32,
    mode: FloodMode,
    source: Source,
    anti_aliased: bool,
}

impl Flood {
    #[must_use]
    pub fn from_point(x: f32, y: f32) -> Flood {
        Flood {
            x,
            y,
            tolerance: DEFAULT_TOLERANCE,
            mode: FloodMode::Contiguous,
            source: Source::Image,
            anti_aliased: true,
        }
    }

    #[must_use]
    pub fn tolerance(mut self, tolerance: f32) -> Flood {
        self.tolerance = match tolerance.is_nan() {
            true => DEFAULT_TOLERANCE,
            false => tolerance.clamp(0.0, 1.0),
        };
        self
    }

    #[must_use]
    pub fn mode(mut self, mode: FloodMode) -> Flood {
        self.mode = mode;
        self
    }

    #[must_use]
    pub fn source(mut self, source: Source) -> Flood {
        self.source = source;
        self
    }

    #[must_use]
    pub fn anti_aliased(mut self, anti_aliased: bool) -> Flood {
        self.anti_aliased = anti_aliased;
        self
    }

    pub(crate) fn sampled(&self) -> Source {
        self.source
    }

    pub(crate) fn seed(&self, width: u32, height: u32) -> Result<(u32, u32)> {
        let (x, y) = (self.x.floor(), self.y.floor());
        match x >= 0.0 && y >= 0.0 && x < width as f32 && y < height as f32 {
            true => Ok((x as u32, y as u32)),
            false => Err(Error::OutsideImage {
                x: self.x,
                y: self.y,
                width,
                height,
            }),
        }
    }

    pub(crate) fn region(&self, pixels: &PixelChange<'_>, within: Option<&[u8]>) -> Result<Region> {
        let (width, height) = (pixels.width, pixels.height);
        let (x, y) = self.seed(width, height)?;
        let seed = pixels.premultiplied(x, y);
        let limit = self.tolerance * self.tolerance;
        let reach = limit * limit;
        let matches = |x: u32, y: u32| {
            within.is_none_or(|within| within[(y * width + x) as usize] > 0)
                && distance(seed, pixels.premultiplied(x, y)) <= reach
        };
        if !matches(x, y) {
            return Ok(Region::default());
        }
        let hard = match self.mode {
            FloodMode::Contiguous => contiguous(width, height, (x, y), matches),
            FloodMode::Global => global(width, height, matches),
        };
        match self.anti_aliased {
            true => smoothed(&hard, Area::sized(width, height)),
            false => Ok(hard),
        }
    }
}

pub(crate) struct PixelChange<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: &'a [u8],
}

impl PixelChange<'_> {
    fn premultiplied(&self, x: u32, y: u32) -> [f32; 4] {
        let at = (y as usize * self.width as usize + x as usize) * 4;
        let [r, g, b, a] = [0, 1, 2, 3].map(|channel| f32::from(self.rgba[at + channel]) / 255.0);
        [r * a, g * a, b * a, a]
    }
}

fn distance(one: [f32; 4], other: [f32; 4]) -> f32 {
    one.iter()
        .zip(&other)
        .map(|(one, other)| (one - other) * (one - other))
        .sum::<f32>()
        / 4.0
}

fn contiguous(
    width: u32,
    height: u32,
    seed: (u32, u32),
    matches: impl Fn(u32, u32) -> bool,
) -> Region {
    let mut visited = vec![0u64; (width as usize * height as usize).div_ceil(64)];
    let at = |x: u32, y: u32| y as usize * width as usize + x as usize;
    let is_visited = |visited: &[u64], index: usize| visited[index / 64] & (1 << (index % 64)) != 0;
    let mut rows: Vec<Vec<(u32, u32)>> = vec![Vec::new(); height as usize];
    let mut pending = vec![seed];
    while let Some((x, y)) = pending.pop() {
        if is_visited(&visited, at(x, y)) || !matches(x, y) {
            continue;
        }
        let mut start = x;
        while start > 0 && !is_visited(&visited, at(start - 1, y)) && matches(start - 1, y) {
            start -= 1;
        }
        let mut end = x + 1;
        while end < width && !is_visited(&visited, at(end, y)) && matches(end, y) {
            end += 1;
        }
        for column in start..end {
            let index = at(column, y);
            visited[index / 64] |= 1 << (index % 64);
        }
        rows[y as usize].push((start, end));
        for neighbour in [y.checked_sub(1), (y + 1 < height).then_some(y + 1)]
            .into_iter()
            .flatten()
        {
            let mut inside = false;
            for column in start..end {
                let open =
                    !is_visited(&visited, at(column, neighbour)) && matches(column, neighbour);
                if open && !inside {
                    pending.push((column, neighbour));
                }
                inside = open;
            }
        }
    }
    let top = rows.iter().position(|row| !row.is_empty()).unwrap_or(0);
    let mut builder = Builder::new(top as u32);
    for row in &mut rows[top..] {
        row.sort_unstable();
        for &(start, end) in row.iter() {
            builder.push(start, end, Span::FULL);
        }
        builder.end_row();
    }
    builder.finish()
}

fn global(width: u32, height: u32, matches: impl Fn(u32, u32) -> bool) -> Region {
    let mut builder = Builder::new(0);
    for y in 0..height {
        let mut run = None;
        for x in 0..width {
            match (matches(x, y), run) {
                (true, None) => run = Some(x),
                (false, Some(start)) => {
                    builder.push(start, x, Span::FULL);
                    run = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run {
            builder.push(start, width, Span::FULL);
        }
        builder.end_row();
    }
    builder.finish()
}

fn smoothed(hard: &Region, canvas: Area) -> Result<Region> {
    let Some(extent) = hard.extent() else {
        return Ok(Region::default());
    };
    let area = widened(extent, 1, canvas);
    let raster = Raster::rasterize(hard, area)?;
    let mut soft = Raster::blank(area)?;
    let (width, height) = (area.width as i64, area.height as i64);
    let inside = |x: i64, y: i64| {
        u32::from(
            raster.values[(y.clamp(0, height - 1) * width + x.clamp(0, width - 1)) as usize] > 0,
        )
    };
    for y in 0..height {
        for x in 0..width {
            let weighed: u32 = WEIGHTS
                .iter()
                .enumerate()
                .flat_map(|(dy, row)| {
                    row.iter()
                        .enumerate()
                        .map(move |(dx, weight)| (dx as i64 - 1, dy as i64 - 1, *weight))
                })
                .map(|(dx, dy, weight)| weight * inside(x + dx, y + dy))
                .sum();
            let level = ((weighed * u32::from(Span::FULL) + WEIGHT / 2) / WEIGHT) as u8;
            soft.values[(y * width + x) as usize] = match inside(x, y) > 0 {
                true => level.max(HALF),
                false => level.min(HALF - 1),
            };
        }
    }
    Ok(soft.region())
}
