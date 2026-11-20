use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rikka_psd::{
    ColorMode, Depth, Document, FillContent, Gradient, GradientFill, GradientKind, GradientStyle,
    PatternFill, Rect,
};

use crate::raster::Raster;
use crate::space::{Space, SpaceKind};

pub(crate) struct GradientLut {
    entries: Vec<([f32; 4], f32)>,
}

fn ease(position: f64, midpoint: f64, smoothness: f64) -> f64 {
    let midpoint = midpoint.clamp(0.001, 0.999);
    let mapped = if position < midpoint {
        0.5 * position / midpoint
    } else {
        0.5 + 0.5 * (position - midpoint) / (1.0 - midpoint)
    };
    let smooth = mapped * mapped * (3.0 - 2.0 * mapped);
    mapped + (smooth - mapped) * smoothness.clamp(0.0, 1.0)
}

fn sample_stops<T: Copy>(
    stops: &[(f64, f64, T)],
    t: f64,
    mix: impl Fn(T, T, f64) -> T,
    smoothness: f64,
) -> Option<T> {
    let first = stops.first()?;
    if t <= first.0 {
        return Some(first.2);
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.0 {
            let span = b.0 - a.0;
            let local = if span <= 0.0 { 1.0 } else { (t - a.0) / span };
            return Some(mix(a.2, b.2, ease(local, a.1, smoothness)));
        }
    }
    stops.last().map(|stop| stop.2)
}

struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

impl GradientLut {
    pub(crate) fn new(gradient: &Gradient, space: &Space, reverse: bool) -> Self {
        let mut entries = Vec::with_capacity(256);
        match &gradient.kind {
            GradientKind::Solid {
                smoothness,
                colors,
                opacities,
            } => {
                let mut color_stops: Vec<(f64, f64, [f64; 3])> = colors
                    .iter()
                    .map(|stop| (stop.location, stop.midpoint, stop.color.to_rgb()))
                    .collect();
                color_stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut opacity_stops: Vec<(f64, f64, f64)> = opacities
                    .iter()
                    .map(|stop| (stop.location, stop.midpoint, stop.opacity))
                    .collect();
                opacity_stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                for index in 0..256 {
                    let t = index as f64 / 255.0;
                    let rgb = sample_stops(
                        &color_stops,
                        t,
                        |a, b, u| {
                            [
                                a[0] + (b[0] - a[0]) * u,
                                a[1] + (b[1] - a[1]) * u,
                                a[2] + (b[2] - a[2]) * u,
                            ]
                        },
                        *smoothness,
                    )
                    .unwrap_or([0.0; 3]);
                    let alpha =
                        sample_stops(&opacity_stops, t, |a, b, u| a + (b - a) * u, *smoothness)
                            .unwrap_or(1.0);
                    let mut color = [0.0f32; 4];
                    space.authored_rgb(rgb.map(|value| value as f32), &mut color);
                    entries.push((color, alpha as f32));
                }
            }
            GradientKind::Noise {
                roughness,
                seed,
                minimum,
                maximum,
                add_transparency,
                ..
            } => {
                let mut noise = Noise(*seed as u64 ^ 0x5851_f42d_4c95_7f2d);
                let count = (4.0 + roughness * 28.0).round() as usize;
                let controls: Vec<([f64; 3], f64)> = (0..count.max(2))
                    .map(|_| {
                        let mut rgb = [0.0; 3];
                        for (channel, slot) in rgb.iter_mut().enumerate() {
                            *slot = minimum[channel]
                                + (maximum[channel] - minimum[channel]) * noise.next();
                        }
                        let alpha = if *add_transparency {
                            minimum[3] + (maximum[3] - minimum[3]) * noise.next()
                        } else {
                            1.0
                        };
                        (rgb, alpha)
                    })
                    .collect();
                for index in 0..256 {
                    let position = index as f64 / 255.0 * (controls.len() - 1) as f64;
                    let low = position.floor() as usize;
                    let high = (low + 1).min(controls.len() - 1);
                    let u = position - low as f64;
                    let rgb = [0, 1, 2].map(|channel| {
                        controls[low].0[channel]
                            + (controls[high].0[channel] - controls[low].0[channel]) * u
                    });
                    let alpha = controls[low].1 + (controls[high].1 - controls[low].1) * u;
                    let mut color = [0.0f32; 4];
                    space.authored_rgb(rgb.map(|value| value.clamp(0.0, 1.0) as f32), &mut color);
                    entries.push((color, alpha as f32));
                }
            }
        }
        if reverse {
            entries.reverse();
        }
        Self { entries }
    }

    pub(crate) fn at(&self, t: f32) -> ([f32; 4], f32) {
        let index = (t.clamp(0.0, 1.0) * 255.0).round() as usize;
        self.entries[index]
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GradientGeometry {
    style: GradientStyle,
    center: (f32, f32),
    direction: (f32, f32),
    half: f32,
    angle: f32,
}

impl GradientGeometry {
    pub(crate) fn new(
        style: GradientStyle,
        angle_degrees: f64,
        scale: f64,
        reference: Rect,
        offset: (f64, f64),
    ) -> Self {
        let radians = angle_degrees.to_radians() as f32;
        let direction = (radians.cos(), -radians.sin());
        let (width, height) = (reference.width() as f32, reference.height() as f32);
        let center = (
            reference.left as f32 + width / 2.0 + offset.0 as f32 * width,
            reference.top as f32 + height / 2.0 + offset.1 as f32 * height,
        );
        let span = if style == GradientStyle::Linear {
            width * direction.0.abs() + height * direction.1.abs()
        } else {
            let ratio = angle_degrees.rem_euclid(90.0) as f32 / 90.0;
            (1.0 - ratio) * width + ratio * height
        };
        Self {
            style,
            center,
            direction,
            half: (span * scale as f32 / 2.0).max(0.5),
            angle: angle_degrees as f32,
        }
    }

    pub(crate) fn at(&self, x: f32, y: f32) -> f32 {
        let (dx, dy) = (x - self.center.0, y - self.center.1);
        let along = dx * self.direction.0 + dy * self.direction.1;
        let across = -dx * self.direction.1 + dy * self.direction.0;
        match self.style {
            GradientStyle::Linear => 0.5 + along / (2.0 * self.half),
            GradientStyle::Radial => (dx * dx + dy * dy).sqrt() / self.half,
            GradientStyle::Reflected => along.abs() / self.half,
            GradientStyle::Diamond => (along.abs() + across.abs()) / self.half,
            GradientStyle::Angle => {
                ((dy.atan2(dx).to_degrees() + self.angle).rem_euclid(360.0)) / 360.0
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct PatternRaster {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<[f32; 5]>,
}

pub(crate) struct PaintContext<'a> {
    pub(crate) document: &'a Document,
    pub(crate) space: Space,
    patterns: RefCell<HashMap<String, Option<Rc<PatternRaster>>>>,
}

impl std::fmt::Debug for PaintContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaintContext")
            .field("space", &self.space)
            .finish_non_exhaustive()
    }
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(document: &'a Document, space: Space) -> Self {
        Self {
            document,
            space,
            patterns: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn pattern(&self, id: &str) -> Option<Rc<PatternRaster>> {
        if let Some(cached) = self.patterns.borrow().get(id) {
            return cached.clone();
        }
        let built = self
            .document
            .patterns()
            .find(|pattern| pattern.id == id)
            .and_then(|pattern| self.build_pattern(pattern))
            .map(Rc::new);
        self.patterns
            .borrow_mut()
            .insert(id.to_owned(), built.clone());
        built
    }

    fn build_pattern(&self, pattern: &rikka_psd::Pattern) -> Option<PatternRaster> {
        let pixels = pattern.pixels().ok()?;
        let color: Vec<Vec<f32>> = pixels
            .color
            .iter()
            .map(rikka_psd::Plane::to_normalized)
            .collect();
        let alpha = pixels.alpha.as_ref().map(rikka_psd::Plane::to_normalized);
        let count = pixels.width as usize * pixels.height as usize;
        let mut out = Vec::with_capacity(count);
        for index in 0..count {
            let rgb = match pattern.mode {
                ColorMode::Rgb => [color[0][index], color[1][index], color[2][index]],
                ColorMode::Indexed => {
                    let palette = pattern.palette.as_deref().unwrap_or(&[]);
                    let entry = palette
                        .get((color[0][index] * 255.0).round() as usize)
                        .copied()
                        .unwrap_or([0; 3]);
                    entry.map(|value| f32::from(value) / 255.0)
                }
                ColorMode::Cmyk => Space::new(SpaceKind::Cmyk, Depth::Eight).to_rgb(&[
                    color[0][index],
                    color[1][index],
                    color[2][index],
                    color[3][index],
                ]),
                ColorMode::Lab => Space::new(SpaceKind::Lab, Depth::Eight).to_rgb(&[
                    color[0][index],
                    color[1][index],
                    color[2][index],
                ]),
                _ => [color[0][index]; 3],
            };
            let mut working = [0.0f32; 5];
            let same_space = matches!(
                (pattern.mode, self.space.kind),
                (ColorMode::Cmyk, SpaceKind::Cmyk)
            );
            if same_space {
                for channel in 0..4 {
                    working[channel] = color[channel][index];
                }
            } else {
                self.space.authored_rgb(rgb, &mut working[..4]);
            }
            working[4] = alpha.as_ref().map_or(1.0, |alpha| alpha[index]);
            out.push(working);
        }
        Some(PatternRaster {
            width: pixels.width,
            height: pixels.height,
            pixels: out,
        })
    }

    pub(crate) fn paint(&self, content: &FillContent, area: Rect, reference: Rect) -> Raster {
        let mut raster = Raster::new(area, self.space.channels());
        let channels = self.space.channels();
        let stride = channels + 1;
        match content {
            FillContent::Solid(color) => {
                let value = self.space.color(color);
                for pixel in raster.data.chunks_exact_mut(stride) {
                    pixel[..channels].copy_from_slice(&value[..channels]);
                    pixel[channels] = 1.0;
                }
            }
            FillContent::Gradient(fill) => self.paint_gradient(&mut raster, fill, reference),
            FillContent::Pattern(fill) => self.paint_pattern(&mut raster, fill, reference),
        }
        raster
    }

    fn paint_gradient(&self, raster: &mut Raster, fill: &GradientFill, reference: Rect) {
        let reference = if fill.align {
            reference
        } else {
            self.document.bounds()
        };
        let lut = GradientLut::new(&fill.gradient, &self.space, fill.reverse);
        let geometry =
            GradientGeometry::new(fill.style, fill.angle, fill.scale, reference, fill.offset);
        let channels = self.space.channels();
        let bounds = raster.bounds;
        for y in bounds.top..bounds.bottom {
            for x in bounds.left..bounds.right {
                let (color, alpha) = lut.at(geometry.at(x as f32 + 0.5, y as f32 + 0.5));
                let index = raster.index(x, y).expect("inside");
                for (slot, value) in raster.data[index..index + channels].iter_mut().zip(&color) {
                    *slot = value * alpha;
                }
                raster.data[index + channels] = alpha;
            }
        }
    }

    pub(crate) fn paint_pattern(&self, raster: &mut Raster, fill: &PatternFill, reference: Rect) {
        let Some(pattern) = self.pattern(&fill.id) else {
            return;
        };
        let origin = if fill.align {
            (reference.left as f32, reference.top as f32)
        } else {
            (0.0, 0.0)
        };
        let scale = (fill.scale as f32).max(0.01);
        let (sin, cos) = (-(fill.angle as f32).to_radians()).sin_cos();
        let channels = self.space.channels();
        let bounds = raster.bounds;
        for y in bounds.top..bounds.bottom {
            for x in bounds.left..bounds.right {
                let px = x as f32 + 0.5 - origin.0 - fill.phase.0 as f32;
                let py = y as f32 + 0.5 - origin.1 - fill.phase.1 as f32;
                let u = (px * cos - py * sin) / scale;
                let v = (px * sin + py * cos) / scale;
                let column = (u.floor() as i64).rem_euclid(i64::from(pattern.width)) as usize;
                let row = (v.floor() as i64).rem_euclid(i64::from(pattern.height)) as usize;
                let sample = pattern.pixels[row * pattern.width as usize + column];
                let alpha = sample[4];
                let index = raster.index(x, y).expect("inside");
                for (slot, value) in raster.data[index..index + channels].iter_mut().zip(&sample) {
                    *slot = value * alpha;
                }
                raster.data[index + channels] = alpha;
            }
        }
    }
}
