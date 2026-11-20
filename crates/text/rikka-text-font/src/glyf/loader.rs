use crate::glyf::glyph::{Anchor, Component, Glyph};
use crate::glyf::loca::Glyf;
use crate::outline::bounds::Bounds;
use crate::outline::scratch::OutlineScratch;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::tables::long_metrics::LongMetrics;
use crate::variations::gvar::Gvar;
use rikka_types::Point;

const MAX_DEPTH: usize = 32;
const MAX_GLYPH_VISITS: usize = 2048;
const MAX_POINTS: usize = 1 << 20;
const PHANTOM_COUNT: usize = 4;

pub(crate) type Phantoms = [Point; PHANTOM_COUNT];

pub(crate) struct Loader<'s, 'a> {
    glyf: Glyf<'a>,
    gvar: Option<Gvar<'a>>,
    hmtx: Option<LongMetrics<'a>>,
    vmtx: Option<LongMetrics<'a>>,
    default_vertical_advance: f32,
    coords: &'s [F2Dot14],
    scratch: &'s mut OutlineScratch,
    visits: usize,
}

impl<'s, 'a> Loader<'s, 'a> {
    pub(crate) fn new(
        font: &Font<'a>,
        coords: &'s [F2Dot14],
        scratch: &'s mut OutlineScratch,
    ) -> Option<Self> {
        let default_vertical_advance = font.hhea().map_or(f32::from(font.units_per_em()), |hhea| {
            f32::from(hhea.ascender()) - f32::from(hhea.descender())
        });
        Some(Self {
            glyf: font.glyf()?,
            gvar: if coords.is_empty() { None } else { font.gvar() },
            hmtx: font.hmtx(),
            vmtx: font.vmtx(),
            default_vertical_advance,
            coords,
            scratch,
            visits: 0,
        })
    }

    pub(crate) fn load_outline(&mut self, glyph: GlyphId) -> Option<()> {
        self.scratch.clear();
        let phantoms = self.load(glyph, 0)?;
        let shift = phantoms[0].x;
        if shift != 0.0 {
            for point in &mut self.scratch.points {
                point.x -= shift;
            }
        }
        Some(())
    }

    fn phantoms(&self, glyph: GlyphId, bounds: Bounds) -> Phantoms {
        let advance = self
            .hmtx
            .and_then(|m| m.advance(glyph))
            .map_or(0.0, f32::from);
        let lsb = self
            .hmtx
            .and_then(|m| m.side_bearing(glyph))
            .map_or(0.0, f32::from);
        let vertical_advance = self
            .vmtx
            .and_then(|m| m.advance(glyph))
            .map_or(self.default_vertical_advance, f32::from);
        let tsb = self
            .vmtx
            .and_then(|m| m.side_bearing(glyph))
            .map_or(0.0, f32::from);
        let left = bounds.x_min - lsb;
        let top = bounds.y_max + tsb;
        [
            Point::new(left, 0.0),
            Point::new(left + advance, 0.0),
            Point::new(0.0, top),
            Point::new(0.0, top - vertical_advance),
        ]
    }

    fn load(&mut self, glyph: GlyphId, depth: usize) -> Option<Phantoms> {
        self.visits += 1;
        if depth > MAX_DEPTH || self.visits > MAX_GLYPH_VISITS {
            return None;
        }
        let data = self.glyf.glyph_data(glyph)?;
        match Glyph::parse(data)? {
            Glyph::Empty => {
                let mut phantoms = self.phantoms(glyph, Bounds::default());
                self.vary_phantoms(glyph, &mut phantoms);
                Some(phantoms)
            }
            Glyph::Simple(simple) => {
                if self.scratch.points.len() + simple.point_count > MAX_POINTS {
                    return None;
                }
                let base = self.scratch.points.len();
                simple.read_points(&mut self.scratch.flags, &mut self.scratch.points)?;
                self.scratch.local_ends.clear();
                for index in 0..simple.contour_ends.len() / 2 {
                    let end = crate::parse::bytes::Bytes::u16_at(simple.contour_ends, index * 2)?;
                    if usize::from(end) >= simple.point_count {
                        return None;
                    }
                    self.scratch.local_ends.push(end);
                    self.scratch.contour_ends.push(base + usize::from(end));
                }
                let mut phantoms = self.phantoms(glyph, simple.bounds);
                self.vary_simple(glyph, base, &mut phantoms);
                Some(phantoms)
            }
            Glyph::Composite(composite) => {
                let mut phantoms = self.phantoms(glyph, composite.bounds);
                let component_count = composite.components().count();
                let deltas_base = self.scratch.component_deltas.len();
                self.vary_composite(glyph, component_count, &mut phantoms);
                let base = self.scratch.points.len();
                for (index, component) in composite.components().enumerate() {
                    let start = self.scratch.points.len();
                    let component_phantoms = self.load(component.glyph, depth + 1)?;
                    let delta = self
                        .scratch
                        .component_deltas
                        .get(deltas_base + index)
                        .copied()
                        .unwrap_or_default();
                    self.place(&component, base, start, delta)?;
                    if component.use_my_metrics {
                        phantoms = component_phantoms;
                    }
                }
                self.scratch.component_deltas.truncate(deltas_base);
                Some(phantoms)
            }
        }
    }

    fn place(
        &mut self,
        component: &Component,
        base: usize,
        start: usize,
        delta: Point,
    ) -> Option<()> {
        let points = &mut self.scratch.points;
        if !component.is_identity() {
            for point in points.get_mut(start..)? {
                let (x, y) = component.transform(point.x, point.y);
                *point = Point::new(x, y);
            }
        }
        let offset = match component.anchor {
            Anchor::Offset(x, y) => {
                let (x, y) = (x + delta.x, y + delta.y);
                if component.scaled_offset {
                    let (x, y) = component.transform(x, y);
                    Point::new(x, y)
                } else {
                    Point::new(x, y)
                }
            }
            Anchor::Points { parent, child } => {
                let parent = *points.get(base..start)?.get(usize::from(parent))?;
                let child = *points.get(start..)?.get(usize::from(child))?;
                parent - child
            }
        };
        if offset != Point::default() {
            for point in points.get_mut(start..)? {
                *point += offset;
            }
        }
        Some(())
    }

    fn vary_phantoms(&mut self, glyph: GlyphId, phantoms: &mut Phantoms) {
        let Some(tuples) = self.gvar.and_then(|gvar| gvar.tuples(glyph, self.coords)) else {
            return;
        };
        for tuple in tuples {
            if let Some(deltas) = tuple.phantom_deltas(0) {
                for (phantom, delta) in phantoms.iter_mut().zip(deltas) {
                    *phantom += delta;
                }
            }
        }
    }

    fn vary_simple(&mut self, glyph: GlyphId, base: usize, phantoms: &mut Phantoms) {
        let Some(tuples) = self.gvar.and_then(|gvar| gvar.tuples(glyph, self.coords)) else {
            return;
        };
        let scratch = &mut *self.scratch;
        let Some(points) = scratch.points.get_mut(base..) else {
            return;
        };
        scratch.deltas.clear();
        scratch
            .deltas
            .resize(points.len() + phantoms.len(), Point::default());
        for tuple in tuples {
            tuple.add_deltas(
                points,
                Some(&scratch.local_ends),
                &mut scratch.deltas,
                &mut scratch.tuple_deltas,
                &mut scratch.touched,
            );
        }
        let (point_deltas, phantom_deltas) = scratch.deltas.split_at(points.len());
        for (point, delta) in points.iter_mut().zip(point_deltas) {
            *point += *delta;
        }
        for (phantom, delta) in phantoms.iter_mut().zip(phantom_deltas) {
            *phantom += *delta;
        }
    }

    fn vary_composite(&mut self, glyph: GlyphId, component_count: usize, phantoms: &mut Phantoms) {
        let deltas_base = self.scratch.component_deltas.len();
        self.scratch
            .component_deltas
            .resize(deltas_base + component_count, Point::default());
        let Some(tuples) = self.gvar.and_then(|gvar| gvar.tuples(glyph, self.coords)) else {
            return;
        };
        let scratch = &mut *self.scratch;
        scratch.deltas.clear();
        scratch
            .deltas
            .resize(component_count + phantoms.len(), Point::default());
        for tuple in tuples {
            tuple.add_deltas(
                &[],
                None,
                &mut scratch.deltas,
                &mut scratch.tuple_deltas,
                &mut scratch.touched,
            );
        }
        let (component_deltas, phantom_deltas) = scratch
            .deltas
            .split_at(component_count.min(scratch.deltas.len()));
        if let Some(stack) = scratch.component_deltas.get_mut(deltas_base..) {
            for (slot, delta) in stack.iter_mut().zip(component_deltas) {
                *slot = *delta;
            }
        }
        for (phantom, delta) in phantoms.iter_mut().zip(phantom_deltas) {
            *phantom += *delta;
        }
    }
}
