use cut_project::{Format, Placement, Projection};
use rikka_app::prelude::*;
use rikka_figure::{Affine, GRIP_REACH, Grip, Grips, TURN_GAP};

use super::viewing::Viewing;

const SAME: f32 = 1e-2;
const PROBES: [Point; 3] = [
    Point::new(0.0, 0.0),
    Point::new(1000.0, 0.0),
    Point::new(0.0, 1000.0),
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ItemProjection {
    placing: Affine,
    unplacing: Affine,
    projection: Projection,
    path: Projection,
}

impl ItemProjection {
    pub(crate) fn new(
        projection: Projection,
        path: Projection,
        placement: &Placement,
        format: Format,
    ) -> Option<ItemProjection> {
        let placing = placement.affine(format);
        Some(ItemProjection {
            unplacing: placing.inverse()?,
            placing,
            projection,
            path,
        })
    }

    pub(crate) fn projection(&self) -> &Projection {
        &self.projection
    }

    pub(crate) fn path(&self) -> &Projection {
        &self.path
    }

    pub(crate) fn placing(&self) -> Affine {
        self.placing
    }

    pub(crate) fn to_screen(&self, flat: Point) -> Option<Point> {
        self.projection.map(self.unplacing.apply(flat))
    }

    pub(crate) fn to_plane(&self, at: Point) -> Option<Point> {
        self.projection
            .unmap(at)
            .map(|local| self.placing.apply(local))
    }

    pub(crate) fn all_to_screen(&self, flat: &[Point]) -> Option<Vec<Point>> {
        flat.iter().map(|point| self.to_screen(*point)).collect()
    }

    pub(crate) fn stretch(&self, flat: Point) -> f32 {
        let here = self.to_screen(flat);
        let reach = |step: Point| match (here, self.to_screen(flat + step)) {
            (Some(here), Some(there)) => here.distance(there),
            _ => 1.0,
        };
        reach(Point::new(1.0, 0.0))
            .max(reach(Point::new(0.0, 1.0)))
            .max(f32::EPSILON)
    }

    pub(crate) fn matches(&self, other: &ItemProjection) -> bool {
        PROBES.iter().all(
            |probe| match (self.to_screen(*probe), other.to_screen(*probe)) {
                (Some(one), Some(two)) => one.distance(two) <= SAME,
                _ => false,
            },
        )
    }

    pub(crate) fn turn_scale(&self, grips: &Grips, viewing: &Viewing) -> f32 {
        let Grips::Frame { frame, .. } = grips else {
            return viewing.scale;
        };
        let top = frame.spot(0, -1);
        let (_, down) = frame.axes();
        let fallback = viewing.scale.max(f32::EPSILON);
        match (self.to_screen(top), self.to_screen(top - down)) {
            (Some(top), Some(above)) => (top.distance(above) * viewing.scale).max(f32::EPSILON),
            _ => fallback,
        }
    }

    pub(crate) fn grip_at(
        &self,
        grips: &Grips,
        viewing: &Viewing,
        position: Point,
    ) -> Option<Grip> {
        let gap = TURN_GAP / self.turn_scale(grips, viewing);
        grips.grip_at(position, GRIP_REACH, gap, |spot| {
            match self.to_screen(spot) {
                Some(at) => viewing.to_screen(at),
                None => Point::new(f32::INFINITY, f32::INFINITY),
            }
        })
    }

    pub(crate) fn grip_spots(&self, grips: &Grips, viewing: &Viewing) -> Option<Vec<Point>> {
        let mut spots = grips.outline();
        spots.extend(grips.handles().into_iter().map(|(_, spot)| spot));
        if let Some((top, turn)) = grips.turn(TURN_GAP / self.turn_scale(grips, viewing)) {
            spots.extend([top, turn]);
        }
        self.all_to_screen(&spots)
    }
}
