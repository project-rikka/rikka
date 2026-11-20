use rikka_text_font::{
    Bounds, Brush, ColorPainter, CompositeMode, Font, GlyphId, Location, OutlineScratch, Point,
    Transform,
};

use crate::color::transform_stack::TransformStack;
use crate::outline::path::Outline;

pub(crate) struct BoundsPainter<'p, 'f> {
    pub(crate) font: &'p Font<'f>,
    pub(crate) location: &'p Location,
    pub(crate) scratch: &'p mut OutlineScratch,
    pub(crate) outline: &'p mut Outline,
    pub(crate) transforms: &'p mut TransformStack,
    pub(crate) clips: &'p mut Vec<Bounds>,
    pub(crate) total: Option<Bounds>,
}

impl BoundsPainter<'_, '_> {
    fn push_clip(&mut self, bounds: Bounds) {
        let clipped = match self.clips.last() {
            Some(parent) => parent.intersection(&bounds),
            None => bounds,
        };
        self.clips.push(clipped);
    }

    fn transformed_bounds(&self, points: impl Iterator<Item = Point>) -> Bounds {
        let transform = self.transforms.current();
        let mut bounds = Bounds::EMPTY;
        for point in points {
            let point = transform.apply(point);
            bounds.include(point.x, point.y);
        }
        bounds
    }
}

pub(crate) fn has_no_area(bounds: &Bounds) -> bool {
    bounds.x_min >= bounds.x_max || bounds.y_min >= bounds.y_max
}

fn corners(bounds: Bounds) -> impl Iterator<Item = Point> {
    [
        Point::new(bounds.x_min, bounds.y_min),
        Point::new(bounds.x_max, bounds.y_min),
        Point::new(bounds.x_max, bounds.y_max),
        Point::new(bounds.x_min, bounds.y_max),
    ]
    .into_iter()
}

impl ColorPainter for BoundsPainter<'_, '_> {
    fn push_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }

    fn push_clip_glyph(&mut self, glyph: GlyphId) {
        let current = self.transforms.current();
        let bounds = if current.xy == 0.0 && current.yx == 0.0 {
            self.font
                .glyph_bounds(glyph, self.location, self.scratch)
                .filter(|bounds| !has_no_area(bounds))
                .map_or(Bounds::EMPTY, |bounds| {
                    self.transformed_bounds(corners(bounds))
                })
        } else {
            self.outline.clear();
            let loaded = self
                .font
                .outline(glyph, self.location, self.scratch, self.outline)
                .is_ok();
            if loaded {
                self.transformed_bounds(self.outline.path().points().iter().copied())
            } else {
                Bounds::EMPTY
            }
        };
        self.push_clip(bounds);
    }

    fn push_clip_box(&mut self, clip: Bounds) {
        let bounds = self.transformed_bounds(corners(clip));
        self.push_clip(bounds);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn fill(&mut self, _: Brush<'_>) {
        let Some(&clip) = self.clips.last() else {
            return;
        };
        if has_no_area(&clip) {
            return;
        }
        self.total = Some(match self.total {
            Some(total) => total.union(&clip),
            None => clip,
        });
    }

    fn push_layer(&mut self, _: CompositeMode) {}

    fn pop_layer(&mut self) {}
}
