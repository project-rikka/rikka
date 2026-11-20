use crate::color::color_line::ColorStops;
use crate::color::colr::{ColorGlyph, ColorGlyphPaint, Colr};
use crate::color::colr_variations::ColrVariations;
use crate::color::composite_mode::CompositeMode;
use crate::color::paint::{Paint, PaintId, parse_paint};
use crate::color::painter::{Brush, ColorPainter};
use crate::error::{Error, Result};
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::glyph_id::GlyphId;
use rikka_types::Point;

const MAX_DEPTH: usize = 64;
const MAX_PAINTS: u32 = 1 << 16;

struct Walker<'c, 'p, P> {
    colr: Colr<'c>,
    variations: ColrVariations<'c>,
    coords: &'c [F2Dot14],
    painter: &'p mut P,
    path: [PaintId; MAX_DEPTH],
    depth: usize,
    budget: u32,
}

impl<'a> ColorGlyph<'a> {
    pub fn paint(&self, coords: &[F2Dot14], painter: &mut impl ColorPainter) -> Result<()> {
        match self.paint {
            ColorGlyphPaint::Layers { first, count } => {
                for index in first..first.saturating_add(count) {
                    let (glyph, color) = self.colr.layer(index).ok_or(Error::Malformed)?;
                    painter.push_clip_glyph(glyph);
                    painter.fill(Brush::Solid { color, alpha: 1.0 });
                    painter.pop_clip();
                }
                Ok(())
            }
            ColorGlyphPaint::Graph(root) => {
                let mut walker = Walker {
                    colr: self.colr,
                    variations: self.colr.variations(coords),
                    coords,
                    painter,
                    path: [PaintId(0); MAX_DEPTH],
                    depth: 0,
                    budget: MAX_PAINTS,
                };
                walker.clipped_glyph(self.glyph, root)
            }
        }
    }
}

impl<'c, P: ColorPainter> Walker<'c, '_, P> {
    fn clipped_glyph(&mut self, glyph: GlyphId, root: PaintId) -> Result<()> {
        match self.colr.clip_box(glyph, self.coords) {
            Some(clip) => {
                self.painter.push_clip_box(clip);
                let result = self.walk(root);
                self.painter.pop_clip();
                result
            }
            None => self.walk(root),
        }
    }

    fn walk(&mut self, id: PaintId) -> Result<()> {
        if self.path[..self.depth].contains(&id) {
            return Err(Error::PaintCycle);
        }
        if self.depth == MAX_DEPTH || self.budget == 0 {
            return Err(Error::PaintTooComplex);
        }
        self.budget -= 1;
        self.path[self.depth] = id;
        self.depth += 1;
        let result = self.visit(id);
        self.depth -= 1;
        result
    }

    fn visit(&mut self, id: PaintId) -> Result<()> {
        let paint = parse_paint(self.colr_data(), id, self.variations).ok_or(Error::Malformed)?;
        match paint {
            Paint::ColrLayers { first, count } => {
                for index in first..first.saturating_add(u32::from(count)) {
                    let layer = self.colr.layer_paint(index).ok_or(Error::Malformed)?;
                    self.walk(layer)?;
                }
                Ok(())
            }
            Paint::Solid { color, alpha } => {
                self.painter.fill(Brush::Solid { color, alpha });
                Ok(())
            }
            Paint::LinearGradient {
                stops,
                extend,
                p0,
                p1,
                p2,
            } => {
                match linear_end(p0, p1, p2) {
                    Some(end) => self.painter.fill(Brush::LinearGradient {
                        start: p0,
                        end,
                        stops,
                        extend,
                    }),
                    None => self.fill_lowest_stop(stops),
                }
                Ok(())
            }
            Paint::RadialGradient {
                stops,
                extend,
                c0,
                r0,
                c1,
                r1,
            } => {
                self.painter.fill(Brush::RadialGradient {
                    start_center: c0,
                    start_radius: r0,
                    end_center: c1,
                    end_radius: r1,
                    stops,
                    extend,
                });
                Ok(())
            }
            Paint::SweepGradient {
                stops,
                extend,
                center,
                start_degrees,
                end_degrees,
            } => {
                self.painter.fill(Brush::SweepGradient {
                    center,
                    start_degrees,
                    end_degrees,
                    stops,
                    extend,
                });
                Ok(())
            }
            Paint::Glyph { glyph, paint } => {
                self.painter.push_clip_glyph(glyph);
                let result = self.walk(paint);
                self.painter.pop_clip();
                result
            }
            Paint::ColrGlyph { glyph } => match self.colr.base_glyph_paint(glyph) {
                Some(root) => self.clipped_glyph(glyph, root),
                None => Ok(()),
            },
            Paint::Transform { transform, paint } => {
                self.painter.push_transform(transform);
                let result = self.walk(paint);
                self.painter.pop_transform();
                result
            }
            Paint::Composite {
                source,
                mode,
                backdrop,
            } => {
                self.painter.push_layer(CompositeMode::SourceOver);
                let mut result = self.walk(backdrop);
                if result.is_ok() {
                    self.painter.push_layer(mode);
                    result = self.walk(source);
                    self.painter.pop_layer();
                }
                self.painter.pop_layer();
                result
            }
        }
    }

    fn colr_data(&self) -> &'c [u8] {
        self.colr.data()
    }

    fn fill_lowest_stop(&mut self, stops: ColorStops<'_>) {
        if let Some(stop) = stops.lowest() {
            self.painter.fill(Brush::Solid {
                color: stop.color,
                alpha: stop.alpha,
            });
        }
    }
}

fn linear_end(p0: Point, p1: Point, p2: Point) -> Option<Point> {
    let (ax, ay) = (p1.x - p0.x, p1.y - p0.y);
    let (bx, by) = (p2.x - p0.x, p2.y - p0.y);
    if ax * by - ay * bx == 0.0 {
        return None;
    }
    let (nx, ny) = (by, -bx);
    let scale = (ax * nx + ay * ny) / (nx * nx + ny * ny);
    Some(Point::new(p0.x + nx * scale, p0.y + ny * scale))
}
