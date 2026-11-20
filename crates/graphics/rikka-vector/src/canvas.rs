use rikka_simd::Level;

use crate::blend::{BlendMode, composite};
use crate::color::{Rgba, load, store};
use crate::error::{Error, Result};
use crate::fill::rasterizer::{Antialiasing, Area, Contours, FILL_FLATNESS, Rasterizer};
use crate::fill::rule::FillRule;
use crate::flatten::polylines::Bounds;
use crate::mask::Mask;
use crate::paint::Paint;
use crate::paint::color_line::ColorLine;
use crate::paint::shader::Shading;
use crate::path::Path;
use crate::pixmap::Pixmap;
use crate::stroke::stroker::Stroker;
use crate::stroke::style::Stroke;
use crate::transform::Transform;

const MAX_FLATTENED: usize = 1 << 21;
const BAND_CELLS: usize = 1 << 18;
const FARTHEST: f32 = 1.0e7;
const OPAQUE: f32 = 1.0 - 0.5 / 255.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Composite<'m> {
    pub mode: BlendMode,
    pub opacity: f32,
    pub mask: Option<Stencil<'m>>,
}

impl Composite<'_> {
    pub const OVER: Composite<'static> = Composite {
        mode: BlendMode::SourceOver,
        opacity: 1.0,
        mask: None,
    };

    #[must_use]
    pub const fn blending(mode: BlendMode) -> Composite<'static> {
        Composite {
            mode,
            opacity: 1.0,
            mask: None,
        }
    }

    #[must_use]
    pub const fn faded(opacity: f32) -> Composite<'static> {
        Composite {
            mode: BlendMode::SourceOver,
            opacity,
            mask: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stencil<'m> {
    pub mask: &'m Mask,
    pub left: i32,
    pub top: i32,
}

#[derive(Debug, Default)]
struct Layer {
    area: Area,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Clip {
    area: Area,
    mask: Option<usize>,
}

#[derive(Debug, Default)]
struct Coverage {
    area: Area,
    values: Vec<u8>,
}

#[derive(Debug)]
pub struct Canvas {
    rasterizer: Rasterizer,
    flatness: f32,
    antialiasing: Antialiasing,
    stroker: Stroker,
    outline: Path,
    line: ColorLine,
    layers: Vec<Layer>,
    active_layers: usize,
    masks: Vec<Coverage>,
    active_masks: usize,
    clips: Vec<Clip>,
    coverage: Vec<f32>,
    shaded: Vec<Rgba>,
}

fn intersect(one: Area, other: Area) -> Area {
    let left = one.left.max(other.left);
    let top = one.top.max(other.top);
    let right = (one.left + one.width as i32).min(other.left + other.width as i32);
    let bottom = (one.top + one.height as i32).min(other.top + other.height as i32);
    match right > left && bottom > top {
        true => Area {
            left,
            top,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        },
        false => Area {
            left: one.left,
            top: one.top,
            width: 0,
            height: 0,
        },
    }
}

fn covering(bounds: &Bounds) -> Area {
    let low = |value: f32| value.clamp(-FARTHEST, FARTHEST).floor() as i32;
    let high = |value: f32| value.clamp(-FARTHEST, FARTHEST).ceil() as i32;
    let (left, top) = (low(bounds.x_min), low(bounds.y_min));
    Area {
        left,
        top,
        width: (high(bounds.x_max) - left).max(0) as u32,
        height: (high(bounds.y_max) - top).max(0) as u32,
    }
}

impl Coverage {
    #[inline]
    fn row(&self, y: i32, left: i32, width: usize) -> &[u8] {
        let start = (y - self.area.top) as usize * self.area.width as usize
            + (left - self.area.left) as usize;
        &self.values[start..start + width]
    }
}

impl Layer {
    #[inline]
    fn row_mut(&mut self, y: i32, left: i32, width: usize) -> &mut [u8] {
        let start = ((y - self.area.top) as usize * self.area.width as usize
            + (left - self.area.left) as usize)
            * 4;
        &mut self.pixels[start..start + width * 4]
    }

    #[inline]
    fn row(&self, y: i32, left: i32, width: usize) -> &[u8] {
        let start = ((y - self.area.top) as usize * self.area.width as usize
            + (left - self.area.left) as usize)
            * 4;
        &self.pixels[start..start + width * 4]
    }
}

enum Source<'s> {
    Solid(Rgba),
    Shaded(&'s [Rgba]),
    Pixels(&'s [u8]),
}

struct Row<'r> {
    source: Source<'r>,
    coverage: Option<&'r [f32]>,
    clip: Option<&'r [u8]>,
    stencil: Option<&'r [u8]>,
    mode: BlendMode,
    opacity: f32,
}

impl Row<'_> {
    fn blend(&self, target: &mut [u8]) {
        const UNIT: f32 = 1.0 / 255.0;
        for (index, pixel) in target.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let mut amount = self.opacity;
            if let Some(coverage) = self.coverage {
                amount *= coverage[index];
            }
            if let Some(clip) = self.clip {
                amount *= f32::from(clip[index]) * UNIT;
            }
            if let Some(stencil) = self.stencil {
                amount *= f32::from(stencil[index]) * UNIT;
            }
            if amount <= 0.0 {
                continue;
            }
            let source = match self.source {
                Source::Solid(color) => color,
                Source::Shaded(colors) => colors[index],
                Source::Pixels(pixels) => {
                    let at = index * 4;
                    load([pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]])
                }
            };
            if self.mode == BlendMode::SourceOver {
                let alpha = source[3] * amount;
                if alpha <= 0.0 {
                    continue;
                }
                if alpha >= OPAQUE {
                    *pixel = store(source);
                    continue;
                }
                let keep = 1.0 - alpha;
                let backdrop = load(*pixel);
                *pixel = store([
                    source[0] * amount + backdrop[0] * keep,
                    source[1] * amount + backdrop[1] * keep,
                    source[2] * amount + backdrop[2] * keep,
                    alpha + backdrop[3] * keep,
                ]);
                continue;
            }
            let backdrop = load(*pixel);
            let result = composite(self.mode, source, backdrop);
            *pixel = store(std::array::from_fn(|channel| {
                backdrop[channel] + (result[channel] - backdrop[channel]) * amount
            }));
        }
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::with_level(Level::detect())
    }
}

impl Canvas {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_level(level: Level) -> Self {
        Self {
            rasterizer: Rasterizer::with_level(level),
            flatness: FILL_FLATNESS,
            antialiasing: Antialiasing::Enabled,
            stroker: Stroker::default(),
            outline: Path::default(),
            line: ColorLine::default(),
            layers: Vec::new(),
            active_layers: 0,
            masks: Vec::new(),
            active_masks: 0,
            clips: Vec::new(),
            coverage: Vec::new(),
            shaded: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_flatness(mut self, tolerance: f32) -> Self {
        self.flatness = tolerance;
        self
    }

    pub fn set_antialiasing(&mut self, antialiasing: Antialiasing) {
        self.antialiasing = antialiasing;
    }

    pub fn begin(&mut self, width: u32, height: u32) {
        self.active_layers = 0;
        self.active_masks = 0;
        self.clips.clear();
        let area = Area {
            left: 0,
            top: 0,
            width,
            height,
        };
        self.clips.push(Clip { area, mask: None });
        self.activate_layer(area);
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.layers.first().map_or(0, |root| root.area.width)
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.layers.first().map_or(0, |root| root.area.height)
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        self.layers.first().map_or(&[], |root| &root.pixels)
    }

    pub fn to_pixmap(&self) -> Result<Pixmap> {
        Pixmap::from_premultiplied(self.width(), self.height(), self.pixels().to_vec())
    }

    #[must_use]
    pub fn clip_area(&self) -> Area {
        self.current_clip().area
    }

    pub fn fill(
        &mut self,
        path: &Path,
        transform: &Transform,
        rule: FillRule,
        paint: &Paint<'_>,
    ) -> Result<()> {
        self.rasterizer
            .flatten_within(path, transform, self.flatness, MAX_FLATTENED)?;
        self.fill_flattened(rule, Contours::Overlapping, paint, transform)
    }

    pub fn stroke(
        &mut self,
        path: &Path,
        transform: &Transform,
        stroke: &Stroke<'_>,
        paint: &Paint<'_>,
    ) -> Result<()> {
        let scale = transform.max_scale();
        if scale <= 0.0 || !scale.is_finite() {
            return Ok(());
        }
        self.stroker
            .outline(path, stroke, self.flatness / scale, &mut self.outline);
        if self.outline.is_truncated() {
            return Err(Error::Truncated);
        }
        self.rasterizer
            .flatten_within(&self.outline, transform, self.flatness, MAX_FLATTENED)?;
        self.fill_flattened(FillRule::NonZero, Contours::Overlapping, paint, transform)
    }

    pub fn paint(&mut self, paint: &Paint<'_>, transform: &Transform) {
        let clip = self.current_clip();
        let Some(shading) = Shading::new(paint, transform, &mut self.line) else {
            return;
        };
        let width = clip.area.width as usize;
        let mask = clip.mask.map(|index| &self.masks[index]);
        let layer = &mut self.layers[self.active_layers - 1];
        let shaded = &mut self.shaded;
        for y in clip.area.top..clip.area.top + clip.area.height as i32 {
            let source = shade(&shading, &self.line, shaded, clip.area.left, y, width);
            Row {
                source,
                coverage: None,
                clip: mask.map(|mask| mask.row(y, clip.area.left, width)),
                stencil: None,
                mode: BlendMode::SourceOver,
                opacity: 1.0,
            }
            .blend(layer.row_mut(y, clip.area.left, width));
        }
    }

    pub fn push_clip(
        &mut self,
        path: &Path,
        transform: &Transform,
        rule: FillRule,
        contours: Contours,
    ) -> Result<()> {
        let flattened =
            self.rasterizer
                .flatten_within(path, transform, self.flatness, MAX_FLATTENED);
        let parent = self.current_clip();
        let area = match (&flattened, self.rasterizer.bounds()) {
            (Ok(()), Some(bounds)) if bounds.finite => intersect(parent.area, covering(&bounds)),
            _ => Area::default(),
        };
        if area.pixels() == 0 {
            self.clips.push(Clip {
                area: Area {
                    left: parent.area.left,
                    top: parent.area.top,
                    width: 0,
                    height: 0,
                },
                mask: None,
            });
            return flattened;
        }
        let index = self.claim_mask(area);
        let width = area.width as usize;
        let rows = (BAND_CELLS / width).clamp(1, area.height as usize);
        self.coverage.resize(width * rows, 0.0);
        let (earlier, rest) = self.masks.split_at_mut(index);
        let mask = &mut rest[0];
        let outer = parent.mask.map(|outer| &earlier[outer]);
        let mut failure = None;
        for top in (area.top..area.top + area.height as i32).step_by(rows) {
            let height = rows.min((area.top + area.height as i32 - top) as usize);
            let band = Area {
                left: area.left,
                top,
                width: area.width,
                height: height as u32,
            };
            let coverage = &mut self.coverage[..width * height];
            let covered = coverage_in(
                &mut self.rasterizer,
                self.antialiasing,
                rule,
                contours,
                band,
                coverage,
            );
            if let Err(error) = covered {
                failure = Some(error);
                coverage.fill(0.0);
            }
            for line in 0..height {
                let y = top + line as i32;
                let start = (y - area.top) as usize * width;
                let values = &mut mask.values[start..start + width];
                let covered = &coverage[line * width..(line + 1) * width];
                match outer.map(|outer| outer.row(y, area.left, width)) {
                    Some(outer) => {
                        for ((value, &coverage), &outer) in
                            values.iter_mut().zip(covered).zip(outer)
                        {
                            *value = (coverage * f32::from(outer) + 0.5) as u8;
                        }
                    }
                    None => {
                        for (value, &coverage) in values.iter_mut().zip(covered) {
                            *value = (coverage * 255.0 + 0.5) as u8;
                        }
                    }
                }
            }
        }
        self.clips.push(Clip {
            area,
            mask: Some(index),
        });
        failure.map_or(Ok(()), Err)
    }

    pub fn push_clip_area(&mut self, area: Area) {
        let parent = self.current_clip();
        self.clips.push(Clip {
            area: intersect(parent.area, area),
            mask: parent.mask,
        });
    }

    pub fn push_clip_stencil(&mut self, stencil: Stencil<'_>) {
        let parent = self.current_clip();
        let area = intersect(
            parent.area,
            Area {
                left: stencil.left,
                top: stencil.top,
                width: stencil.mask.width(),
                height: stencil.mask.height(),
            },
        );
        if area.pixels() == 0 {
            self.clips.push(Clip { area, mask: None });
            return;
        }
        let index = self.claim_mask(area);
        let width = area.width as usize;
        let (earlier, rest) = self.masks.split_at_mut(index);
        let mask = &mut rest[0];
        let outer = parent.mask.map(|outer| &earlier[outer]);
        let stride = stencil.mask.width() as usize;
        for y in area.top..area.top + area.height as i32 {
            let start = (y - stencil.top) as usize * stride + (area.left - stencil.left) as usize;
            let given = &stencil.mask.data()[start..start + width];
            let at = (y - area.top) as usize * width;
            let values = &mut mask.values[at..at + width];
            match outer.map(|outer| outer.row(y, area.left, width)) {
                Some(outer) => {
                    for ((value, &given), &outer) in values.iter_mut().zip(given).zip(outer) {
                        *value = ((u32::from(given) * u32::from(outer) + 127) / 255) as u8;
                    }
                }
                None => values.copy_from_slice(given),
            }
        }
        self.clips.push(Clip {
            area,
            mask: Some(index),
        });
    }

    pub fn pop_clip(&mut self) {
        if self.clips.len() <= 1 {
            return;
        }
        let Some(clip) = self.clips.pop() else {
            return;
        };
        let inherited = self.clips.last().and_then(|parent| parent.mask);
        if clip.mask.is_some() && clip.mask != inherited {
            self.active_masks -= 1;
        }
    }

    pub fn push_layer(&mut self) {
        let area = self.current_clip().area;
        self.push_layer_within(area);
    }

    pub fn push_layer_within(&mut self, area: Area) {
        let area = intersect(self.current_clip().area, area);
        self.activate_layer(area);
        self.clips.push(Clip { area, mask: None });
    }

    pub fn push_layer_covering(&mut self, area: Area) {
        self.activate_layer(area);
        self.clips.push(Clip { area, mask: None });
    }

    pub fn layer_mut(&mut self) -> (Area, &mut [u8]) {
        let layer = &mut self.layers[self.active_layers - 1];
        (layer.area, &mut layer.pixels)
    }

    pub fn pop_layer(&mut self, composite: &Composite<'_>) {
        if self.active_layers <= 1 {
            return;
        }
        self.clips.pop();
        let clip = self.current_clip();
        let (below, above) = self.layers.split_at_mut(self.active_layers - 1);
        let source = &above[0];
        let backdrop = &mut below[self.active_layers - 2];
        let mut area = intersect(clip.area, source.area);
        if let Some(stencil) = composite.mask {
            area = intersect(
                area,
                Area {
                    left: stencil.left,
                    top: stencil.top,
                    width: stencil.mask.width(),
                    height: stencil.mask.height(),
                },
            );
        }
        let width = area.width as usize;
        let mask = clip.mask.map(|index| &self.masks[index]);
        for y in area.top..area.top + area.height as i32 {
            let stencil = composite.mask.map(|stencil| {
                let start = (y - stencil.top) as usize * stencil.mask.width() as usize
                    + (area.left - stencil.left) as usize;
                &stencil.mask.data()[start..start + width]
            });
            Row {
                source: Source::Pixels(source.row(y, area.left, width)),
                coverage: None,
                clip: mask.map(|mask| mask.row(y, area.left, width)),
                stencil,
                mode: composite.mode,
                opacity: composite.opacity.clamp(0.0, 1.0),
            }
            .blend(backdrop.row_mut(y, area.left, width));
        }
        self.active_layers -= 1;
    }

    fn fill_flattened(
        &mut self,
        rule: FillRule,
        contours: Contours,
        paint: &Paint<'_>,
        transform: &Transform,
    ) -> Result<()> {
        let Some(bounds) = self.rasterizer.bounds() else {
            return Ok(());
        };
        if !bounds.finite {
            return Err(Error::TooComplex);
        }
        let clip = self.current_clip();
        let area = intersect(clip.area, covering(&bounds));
        if area.pixels() == 0 {
            return Ok(());
        }
        let Some(shading) = Shading::new(paint, transform, &mut self.line) else {
            return Ok(());
        };
        let width = area.width as usize;
        let rows = (BAND_CELLS / width).clamp(1, area.height as usize);
        self.coverage.resize(width * rows, 0.0);
        let mask = clip.mask.map(|index| &self.masks[index]);
        let layer = &mut self.layers[self.active_layers - 1];
        for top in (area.top..area.top + area.height as i32).step_by(rows) {
            let height = rows.min((area.top + area.height as i32 - top) as usize);
            let band = Area {
                left: area.left,
                top,
                width: area.width,
                height: height as u32,
            };
            let coverage = &mut self.coverage[..width * height];
            coverage_in(
                &mut self.rasterizer,
                self.antialiasing,
                rule,
                contours,
                band,
                coverage,
            )?;
            for line in 0..height {
                let y = top + line as i32;
                let source = shade(&shading, &self.line, &mut self.shaded, area.left, y, width);
                Row {
                    source,
                    coverage: Some(&coverage[line * width..(line + 1) * width]),
                    clip: mask.map(|mask| mask.row(y, area.left, width)),
                    stencil: None,
                    mode: BlendMode::SourceOver,
                    opacity: 1.0,
                }
                .blend(layer.row_mut(y, area.left, width));
            }
        }
        Ok(())
    }

    fn activate_layer(&mut self, area: Area) {
        if self.layers.len() == self.active_layers {
            self.layers.push(Layer::default());
        }
        let layer = &mut self.layers[self.active_layers];
        layer.area = area;
        layer.pixels.clear();
        layer.pixels.resize(area.pixels() * 4, 0);
        self.active_layers += 1;
    }

    fn claim_mask(&mut self, area: Area) -> usize {
        if self.masks.len() == self.active_masks {
            self.masks.push(Coverage::default());
        }
        let index = self.active_masks;
        let mask = &mut self.masks[index];
        mask.area = area;
        mask.values.clear();
        mask.values.resize(area.pixels(), 0);
        self.active_masks += 1;
        index
    }

    #[inline]
    fn current_clip(&self) -> Clip {
        self.clips[self.clips.len() - 1]
    }
}

fn coverage_in(
    rasterizer: &mut Rasterizer,
    antialiasing: Antialiasing,
    rule: FillRule,
    contours: Contours,
    band: Area,
    out: &mut [f32],
) -> Result<()> {
    match antialiasing {
        Antialiasing::Enabled => rasterizer.coverage(rule, contours, band, out),
        Antialiasing::Disabled => {
            rasterizer.aliased_coverage(rule, band, out);
            Ok(())
        }
    }
}

fn shade<'s>(
    shading: &Shading<'_>,
    line: &ColorLine,
    shaded: &'s mut Vec<Rgba>,
    left: i32,
    y: i32,
    width: usize,
) -> Source<'s> {
    if let Some(color) = shading.solid() {
        return Source::Solid(color);
    }
    shaded.clear();
    let center = y as f32 + 0.5;
    shaded.extend(
        (0..width).map(|column| shading.at(line, (left + column as i32) as f32 + 0.5, center)),
    );
    Source::Shaded(shaded)
}
