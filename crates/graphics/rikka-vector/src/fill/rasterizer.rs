use rikka_simd::{Level, Simd};

use crate::error::{Error, Result};
use crate::fill::accumulator::Accumulator;
use crate::fill::rule::FillRule;
use crate::fill::sweep::Sweep;
use crate::flatten::flattener::MAX_FLATTENED;
use crate::flatten::polylines::{Bounds, Polylines};
use crate::path::Path;
use crate::rounding::{ceil_to_i32, floor_to_i32};
use crate::transform::Transform;

pub const GLYPH_FLATNESS: f32 = 0.0743;
pub const FILL_FLATNESS: f32 = 0.02;

const MAX_SIDE: i32 = 8192;
const MAX_AREA: usize = 1 << 24;
const MAX_OFFSET: f32 = 1.0e6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Area {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    #[inline]
    #[must_use]
    pub fn pixels(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Antialiasing {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Contours {
    #[default]
    Simple,
    Overlapping,
}

#[derive(Debug, Default)]
pub struct Rasterizer {
    level: Level,
    polylines: Polylines,
    accumulator: Accumulator,
    sweep: Sweep,
}

impl Rasterizer {
    #[must_use]
    pub fn new() -> Self {
        Self::with_level(Level::detect())
    }

    #[must_use]
    pub fn with_level(level: Level) -> Self {
        Self {
            level,
            polylines: Polylines::default(),
            accumulator: Accumulator::default(),
            sweep: Sweep::default(),
        }
    }

    #[inline]
    #[must_use]
    pub fn level(&self) -> Level {
        self.level
    }

    #[inline(always)]
    pub fn polylines_mut(&mut self) -> &mut Polylines {
        &mut self.polylines
    }

    pub fn flatten(&mut self, path: &Path, transform: &Transform, tolerance: f32) -> Result<()> {
        self.flatten_within(path, transform, tolerance, MAX_FLATTENED)
    }

    pub fn flatten_within(
        &mut self,
        path: &Path,
        transform: &Transform,
        tolerance: f32,
        limit: usize,
    ) -> Result<()> {
        match self.polylines.flatten(path, transform, tolerance, limit) {
            true => Ok(()),
            false => Err(Error::TooComplex),
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Option<Bounds> {
        let polylines = &self.polylines;
        rikka_simd::dispatch!(self.level, simd => bounds_in(simd, polylines))
    }

    pub fn mask<'t>(&mut self, target: impl FnOnce(Area) -> &'t mut [u8]) -> Result<Option<Area>> {
        rikka_simd::dispatch!(self.level, simd => self.mask_in(simd, target))
    }

    #[inline(always)]
    pub fn mask_in<'t, S: Simd>(
        &mut self,
        _: S,
        target: impl FnOnce(Area) -> &'t mut [u8],
    ) -> Result<Option<Area>> {
        let Some(bounds) = self.polylines.bounds::<S::F32s>() else {
            return Ok(None);
        };
        let near = [bounds.x_min, bounds.x_max, bounds.y_min, bounds.y_max]
            .iter()
            .all(|value| value.abs() <= MAX_OFFSET);
        if !bounds.finite || !near {
            return Err(Error::TooComplex);
        }
        if !bounds.is_solid() {
            return Ok(None);
        }
        let left = floor_to_i32(bounds.x_min);
        let top = floor_to_i32(bounds.y_min);
        let width = ceil_to_i32(bounds.x_max) - left;
        let height = ceil_to_i32(bounds.y_max) - top;
        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(Error::TooComplex);
        }
        let area = Area {
            left,
            top,
            width: width as u32,
            height: height as u32,
        };
        if area.pixels() > MAX_AREA {
            return Err(Error::TooComplex);
        }
        self.accumulator
            .begin(area.width as usize, area.height as usize);
        self.accumulator
            .polylines::<S::F32s>(&self.polylines, -(left as f32), -(top as f32));
        if !self.accumulator.accumulate::<S::F32s>() {
            return Err(Error::TooComplex);
        }
        self.accumulator.resolve_mask::<S>(target(area));
        Ok(Some(area))
    }

    pub fn coverage(
        &mut self,
        rule: FillRule,
        contours: Contours,
        area: Area,
        out: &mut [f32],
    ) -> Result<()> {
        let (width, height) = (area.width as usize, area.height as usize);
        debug_assert!(out.len() >= width * height);
        if contours == Contours::Overlapping {
            self.sweep.coverage(&self.polylines, rule, area, out);
            return Ok(());
        }
        self.accumulator.begin(width, height);
        let (dx, dy) = (-(area.left as f32), -(area.top as f32));
        let lines = &self.polylines;
        for contour in lines.contours() {
            self.accumulator.clipped_contour(
                &lines.xs[contour.clone()],
                &lines.ys[contour],
                dx,
                dy,
            );
        }
        let accumulator = &mut self.accumulator;
        if !rikka_simd::dispatch!(self.level, simd => accumulate(simd, accumulator)) {
            return Err(Error::TooComplex);
        }
        self.accumulator.resolve_coverage(rule, out);
        Ok(())
    }

    pub fn aliased_coverage(&mut self, rule: FillRule, area: Area, out: &mut [f32]) {
        debug_assert!(out.len() >= area.pixels());
        self.sweep.aliased(&self.polylines, rule, area, out);
    }
}

#[inline(always)]
fn accumulate<S: Simd>(_: S, accumulator: &mut Accumulator) -> bool {
    accumulator.accumulate::<S::F32s>()
}

#[inline(always)]
fn bounds_in<S: Simd>(_: S, polylines: &Polylines) -> Option<Bounds> {
    polylines.bounds::<S::F32s>()
}
