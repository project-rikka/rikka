use rikka_text_font::{
    Font, GlyphId, Location, OutlineScratch, OutlineSink, Point, Synthesis, Transform,
};
use rikka_vector::Verb;

use crate::distortion::Distortion;
use crate::error::{Error, Result};
use crate::outline::embolden::{embolden, outward_miters};
use crate::outline::flattened::Flattened;
use crate::outline::path::Outline;

const KEPT_SIZES: usize = 4;

#[derive(Clone, Debug, Default)]
pub struct PreparedOutline {
    pub(crate) outline: Outline,
    pub(crate) units_per_em: f32,
    miters: Vec<Point>,
    normals: Vec<Point>,
    miters_ready: bool,
    sizes: Vec<Flattened>,
    used: usize,
    next_replaced: usize,
}

impl PreparedOutline {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_loaded(&self) -> bool {
        self.units_per_em > 0.0
    }

    pub fn heap_bytes(&self) -> usize {
        let floats = self
            .sizes
            .iter()
            .map(|size| {
                size.xs.capacity()
                    + size.ys.capacity()
                    + size.miter_xs.capacity()
                    + size.miter_ys.capacity()
                    + size.ends.capacity() * 2
            })
            .sum::<usize>();
        let points = self.miters.capacity() + self.normals.capacity();
        floats * size_of::<f32>()
            + points * size_of::<Point>()
            + self.outline.path().heap_bytes()
            + self.sizes.capacity() * size_of::<Flattened>()
    }

    pub fn trace(
        &mut self,
        synthesis: Synthesis,
        ppem: f32,
        device: &Transform,
        sink: &mut impl OutlineSink,
    ) -> Result<()> {
        if !self.is_loaded() {
            return Err(Error::NotLoaded);
        }
        if !ppem.is_finite() || ppem <= 0.0 {
            return Err(Error::InvalidRequest);
        }
        let distortion = Distortion::new(synthesis, self.units_per_em);
        let scale = ppem / self.units_per_em;
        let placement = distortion.placement(scale, Point::default()).then(*device);
        if distortion.embolden != 0.0 {
            self.prepare_miters();
        }
        let miters = ready_miters(&self.miters, distortion.embolden != 0.0);
        let points = self.outline.path().points();
        let placed = |index: usize| {
            let point = points[index];
            let point = match miters {
                Some(miters) => Point::new(
                    point.x + miters[index].x * distortion.embolden,
                    point.y + miters[index].y * distortion.embolden,
                ),
                None => point,
            };
            placement.apply(point)
        };
        let mut tracer = Tracer {
            sink,
            start: None,
            open: false,
        };
        let mut at = 0;
        for verb in self.outline.path().verbs() {
            match verb {
                Verb::Move => {
                    tracer.close();
                    tracer.start = Some(placed(at));
                    at += 1;
                }
                Verb::Line => {
                    let to = placed(at);
                    tracer.begin().line_to(to.x, to.y);
                    at += 1;
                }
                Verb::Quad => {
                    let (control, to) = (placed(at), placed(at + 1));
                    tracer.begin().quad_to(control.x, control.y, to.x, to.y);
                    at += 2;
                }
                Verb::Cubic => {
                    let (first, second, to) = (placed(at), placed(at + 1), placed(at + 2));
                    tracer
                        .begin()
                        .cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
                    at += 3;
                }
                Verb::Close => {}
            }
        }
        tracer.close();
        Ok(())
    }

    pub(crate) fn load(
        &mut self,
        font: &Font<'_>,
        glyph: GlyphId,
        location: &Location,
        scratch: &mut OutlineScratch,
    ) -> Result<()> {
        self.outline.clear();
        self.miters.clear();
        self.miters_ready = false;
        self.used = 0;
        self.next_replaced = 0;
        self.units_per_em = 0.0;
        let loaded = font.outline(glyph, location, scratch, &mut self.outline);
        if let Err(error) = loaded {
            self.outline.clear();
            return Err(error.into());
        }
        if self.outline.is_truncated() {
            self.outline.clear();
            return Err(Error::Truncated);
        }
        self.units_per_em = f32::from(font.units_per_em());
        Ok(())
    }

    pub(crate) fn flattened(
        &mut self,
        size_class: i32,
        flatness: f32,
        needs_miters: bool,
        steps: &mut Vec<u16>,
    ) -> Option<&Flattened> {
        if needs_miters {
            self.prepare_miters();
        }
        let found = self.sizes[..self.used]
            .iter()
            .position(|size| size.size_class == size_class);
        let slot = match found {
            Some(slot)
                if !needs_miters || self.miters.is_empty() || self.sizes[slot].has_miters() =>
            {
                return Some(&self.sizes[slot]);
            }
            Some(slot) => slot,
            None if self.used < KEPT_SIZES => {
                if self.sizes.len() == self.used {
                    self.sizes.push(Flattened::default());
                }
                self.used += 1;
                self.used - 1
            }
            None => {
                let slot = self.next_replaced;
                self.next_replaced = (slot + 1) % KEPT_SIZES;
                slot
            }
        };
        let miters = ready_miters(&self.miters, needs_miters);
        let size = &mut self.sizes[slot];
        if !size.build(&self.outline, miters, size_class, flatness, steps) {
            size.size_class = i32::MIN;
            return None;
        }
        Some(size)
    }

    pub(crate) fn embolden_in_place(&mut self, amount: f32) {
        if amount != 0.0 {
            self.prepare_miters();
            embolden(&mut self.outline, &self.miters, amount);
            self.miters_ready = false;
            self.used = 0;
        }
    }

    fn prepare_miters(&mut self) {
        if !self.miters_ready {
            outward_miters(&self.outline, &mut self.normals, &mut self.miters);
            self.miters_ready = true;
        }
    }
}

fn ready_miters(miters: &[Point], needed: bool) -> Option<&[Point]> {
    (needed && !miters.is_empty()).then_some(miters)
}

struct Tracer<'s, S> {
    sink: &'s mut S,
    start: Option<Point>,
    open: bool,
}

impl<S: OutlineSink> Tracer<'_, S> {
    fn begin(&mut self) -> &mut S {
        if let Some(start) = self.start.take() {
            self.sink.move_to(start.x, start.y);
            self.open = true;
        }
        self.sink
    }

    fn close(&mut self) {
        self.start = None;
        if self.open {
            self.sink.close();
            self.open = false;
        }
    }
}
