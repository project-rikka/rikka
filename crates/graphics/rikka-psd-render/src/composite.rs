use std::marker::PhantomData;
use std::ops::Range;

use rikka_psd::{BlendMode, Rect};
use rikka_simd::{Level, Mask as _};

use crate::blend::{Blend, Blending, blend, blending, dissolve_visible};
use crate::lanes::{Kernel, Lanes, Pixels, by_channels, sweep};
use crate::mask::{Mask, coverage_row};
use crate::planar::Planar;
use crate::raster::Raster;
use crate::space::Space;

pub(crate) struct Fade<'a> {
    pub(crate) opacity: f32,
    pub(crate) masks: &'a [Mask],
}

pub(crate) fn composite_raster(
    destination: &mut Raster,
    source: &Raster,
    mode: BlendMode,
    space: &Space,
    coverage: Option<&Mask>,
    opacity: f32,
) {
    let area = source.bounds.intersection(&destination.bounds);
    let paint = Layer::new(source, coverage, opacity, area);
    composite(destination, area, mode, space, paint, None);
}

pub(crate) fn composite_faded(
    canvas: &mut Raster,
    source: &Raster,
    mode: BlendMode,
    space: &Space,
    fill: f32,
    fade: Fade<'_>,
) {
    let area = source.bounds.intersection(&canvas.bounds);
    let paint = Layer::new(source, None, fill, area);
    composite(canvas, area, mode, space, paint, Some(fade));
}

pub(crate) fn composite_color(
    destination: &mut Raster,
    coverage: &Mask,
    color: [f32; 4],
    mode: BlendMode,
    space: &Space,
    opacity: f32,
) {
    let area = coverage.bounds.intersection(&destination.bounds);
    let paint = Fill {
        color,
        coverage,
        opacity,
    };
    composite(destination, area, mode, space, paint, None);
}

fn composite<P: Paint>(
    destination: &mut Raster,
    area: Rect,
    mode: BlendMode,
    space: &Space,
    mut paint: P,
    fade: Option<Fade<'_>>,
) {
    if area.is_empty() {
        return;
    }
    let columns = area.left..area.right;
    let mut source = Planar::new(columns.len());
    let mut factors = vec![0.0; fade.as_ref().map_or(0, |_| columns.len())];
    for y in area.top..area.bottom {
        if let Some(fade) = &fade {
            coverage_row(fade.opacity, fade.masks, y, area.left, &mut factors);
            if factors.iter().all(|&factor| factor <= 0.0) {
                continue;
            }
        }
        paint.unpack(y, columns.clone(), &mut source, space.channels());
        if source.alpha.iter().all(|&alpha| alpha <= 0.0) {
            continue;
        }
        let span = destination.span(y, columns.clone());
        let over = Row {
            pixels: &mut destination.data[span],
            source: &source,
            factors: fade.as_ref().map(|_| factors.as_slice()),
            mode,
            space,
            left: area.left,
            y,
        };
        blending(mode, over);
    }
}

trait Paint {
    fn unpack(&mut self, y: i32, columns: Range<i32>, source: &mut Planar, channels: usize);
}

struct Layer<'a> {
    source: &'a Raster,
    coverage: Option<&'a Mask>,
    opacity: f32,
    factors: Vec<f32>,
}

impl<'a> Layer<'a> {
    fn new(source: &'a Raster, coverage: Option<&'a Mask>, opacity: f32, area: Rect) -> Self {
        Self {
            source,
            coverage,
            opacity,
            factors: vec![0.0; area.width() as usize],
        }
    }
}

impl Paint for Layer<'_> {
    fn unpack(&mut self, y: i32, columns: Range<i32>, source: &mut Planar, channels: usize) {
        if let Some(mask) = self.coverage {
            mask.row_into(y, columns.start, &mut self.factors);
        }
        let span = self.source.span(y, columns);
        let (row, opacity) = (&self.source.data[span], self.opacity);
        let factors = self.coverage.map(|_| self.factors.as_slice());
        let count = self.factors.len();
        by_channels!(channels, C => rikka_simd::dispatch!(Level::detect(), simd => {
            let mut kernel = Straighten::<C> { row, factors, opacity, source };
            sweep(simd, &mut kernel, count)
        }));
    }
}

struct Straighten<'a, const C: usize> {
    row: &'a [f32],
    factors: Option<&'a [f32]>,
    opacity: f32,
    source: &'a mut Planar,
}

impl<const C: usize> Kernel for Straighten<'_, C> {
    #[inline(always)]
    fn run<V: Lanes>(&mut self, at: usize) {
        let lanes = at..at + V::LANES;
        let pixels = Pixels::<V, C>::load(self.row, at);
        let visible = pixels.alpha.simd_gt(V::default());
        for (plane, value) in self.source.color.iter_mut().zip(pixels.color) {
            let straight = V::select(visible, value / pixels.alpha, V::default());
            straight.store(&mut plane[lanes.clone()]);
        }
        let alpha = pixels.alpha * V::splat(self.opacity);
        let alpha = match self.factors {
            Some(factors) => alpha * V::load(&factors[lanes.clone()]),
            None => alpha,
        };
        let alpha = alpha.clamp(V::default(), V::splat(1.0));
        alpha.store(&mut self.source.alpha[lanes]);
    }
}

struct Fill<'a> {
    color: [f32; 4],
    coverage: &'a Mask,
    opacity: f32,
}

impl Paint for Fill<'_> {
    fn unpack(&mut self, y: i32, columns: Range<i32>, source: &mut Planar, channels: usize) {
        for (plane, value) in source.color.iter_mut().zip(self.color).take(channels) {
            plane.fill(value);
        }
        self.coverage.row_into(y, columns.start, &mut source.alpha);
        for alpha in &mut source.alpha {
            *alpha = (*alpha * self.opacity).clamp(0.0, 1.0);
        }
    }
}

struct Row<'a> {
    pixels: &'a mut [f32],
    source: &'a Planar,
    factors: Option<&'a [f32]>,
    mode: BlendMode,
    space: &'a Space,
    left: i32,
    y: i32,
}

impl Blending for Row<'_> {
    type Output = ();

    fn separable<B: Blend>(self) {
        let count = self.source.alpha.len();
        let (pixels, source, factors) = (self.pixels, self.source, self.factors);
        by_channels!(self.space.channels(), C => rikka_simd::dispatch!(Level::detect(), simd => {
            let mut kernel = Over::<B, C> { pixels, source, factors, blend: PhantomData };
            sweep(simd, &mut kernel, count)
        }));
    }

    fn per_pixel(self) {
        let channels = self.space.channels();
        let mut source = [0.0f32; 4];
        let mut backdrop = [0.0f32; 4];
        let mut blended = [0.0f32; 4];
        let mut drawn = [0.0f32; 5];
        let pixels = self.pixels.chunks_exact_mut(channels + 1);
        for (at, (x, pixel)) in (self.left..).zip(pixels).enumerate() {
            let mut alpha = self.source.alpha[at];
            if alpha <= 0.0 {
                continue;
            }
            if self.mode == BlendMode::Dissolve {
                if !dissolve_visible(x, self.y, alpha) {
                    continue;
                }
                alpha = 1.0;
            }
            for (slot, plane) in source.iter_mut().zip(&self.source.color) {
                *slot = plane[at];
            }
            let backdrop_alpha = pixel[channels];
            for (slot, value) in backdrop.iter_mut().zip(&pixel[..channels]) {
                *slot = if backdrop_alpha > 0.0 {
                    value / backdrop_alpha
                } else {
                    0.0
                };
            }
            blend(self.mode, self.space, &backdrop, &source, &mut blended);
            for channel in 0..channels {
                drawn[channel] = (1.0 - backdrop_alpha) * alpha * source[channel]
                    + (1.0 - alpha) * pixel[channel]
                    + alpha * backdrop_alpha * blended[channel];
            }
            drawn[channels] = alpha + backdrop_alpha * (1.0 - alpha);
            let factor = self.factors.map_or(1.0, |factors| factors[at]);
            for (value, toward) in pixel.iter_mut().zip(drawn) {
                *value = match self.factors {
                    None => toward,
                    Some(_) if factor > 0.0 => *value + (toward - *value) * factor,
                    Some(_) => *value,
                };
            }
        }
    }
}

#[inline(always)]
fn faded<V: Lanes>(factors: Option<&[f32]>, at: usize, before: V, after: V) -> V {
    let Some(factors) = factors else {
        return after;
    };
    let factor = V::load(&factors[at..at + V::LANES]);
    V::select(
        factor.simd_gt(V::default()),
        before + (after - before) * factor,
        before,
    )
}

struct Over<'a, B, const C: usize> {
    pixels: &'a mut [f32],
    source: &'a Planar,
    factors: Option<&'a [f32]>,
    blend: PhantomData<B>,
}

impl<B: Blend, const C: usize> Kernel for Over<'_, B, C> {
    #[inline(always)]
    fn run<V: Lanes>(&mut self, at: usize) {
        let lanes = at..at + V::LANES;
        let (zero, one) = (V::default(), V::splat(1.0));
        let alpha = V::load(&self.source.alpha[lanes.clone()]);
        let drawn = alpha.simd_gt(zero);
        if !drawn.any() {
            return;
        }
        let mut pixels = Pixels::<V, C>::load(self.pixels, at);
        let backdrop = pixels.alpha;
        for (value, plane) in pixels.color.iter_mut().zip(&self.source.color) {
            let cs = V::load(&plane[lanes.clone()]);
            let cb = V::select(backdrop.simd_gt(zero), *value / backdrop, zero);
            let blended = match B::CLAMPS {
                true => B::mix(cb.clamp(zero, one), cs.clamp(zero, one)),
                false => B::mix(cb, cs),
            };
            let mixed =
                (one - backdrop) * alpha * cs + (one - alpha) * *value + alpha * backdrop * blended;
            *value = faded(self.factors, at, *value, V::select(drawn, mixed, *value));
        }
        let covered = V::select(drawn, alpha + backdrop * (one - alpha), backdrop);
        pixels.alpha = faded(self.factors, at, backdrop, covered);
        pixels.store(self.pixels, at);
    }
}
