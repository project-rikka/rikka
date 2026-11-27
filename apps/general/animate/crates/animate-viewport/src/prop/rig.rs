use animate_stage::{Lens, PropFrame};
use rikka_app::prelude::*;
use rikka_model::glam::{Quat, Vec3};

use crate::strokes::{RING_POINTS, basis};

pub(crate) const ARM: f32 = 72.0;
pub(crate) const RING: f32 = 96.0;
pub(crate) const PLANE: f32 = 22.0;
pub(crate) const PLANE_SIDE: f32 = 12.0;
const REACH: f32 = 7.0;
const FREE_REACH: f32 = 10.0;
const ARM_START: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Free,
    Axis(usize),
    Plane(usize),
    Ring(usize),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rig {
    pub(crate) lens: Lens,
    pub(crate) origin: Vec3,
    pub(crate) axes: [Vec3; 3],
    pub(crate) turn: Quat,
    pub(crate) stretch: Vec3,
    pub(crate) pixel: f32,
}

impl Rig {
    pub(crate) fn new(frame: &PropFrame, rect: Rect) -> Self {
        let lens = Lens::new(frame.view_projection, rect);
        let (stretch, turn, _) = frame.parent.to_scale_rotation_translation();
        let origin = frame.parent.transform_point3(frame.levels.position());
        let (_, _, pixel) = lens.sideways(origin);
        Self {
            lens,
            origin,
            axes: [turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z],
            turn,
            stretch,
            pixel,
        }
    }

    pub(crate) fn arm(&self, axis: usize) -> Vec3 {
        self.origin + self.axes[axis] * (ARM * self.pixel)
    }

    pub(crate) fn plane_corners(&self, normal: usize) -> [Vec3; 4] {
        let (one, two) = others(normal);
        let near = PLANE * self.pixel;
        let far = (PLANE + PLANE_SIDE) * self.pixel;
        let at = |a: f32, b: f32| self.origin + self.axes[one] * a + self.axes[two] * b;
        [at(near, near), at(far, near), at(far, far), at(near, far)]
    }

    pub(crate) fn ring(&self, axis: usize) -> impl Iterator<Item = Vec3> + '_ {
        let (across, beside) = basis(self.axes[axis]);
        let radius = RING * self.pixel;
        (0..=RING_POINTS).map(move |step| {
            let turn = std::f32::consts::TAU * step as f32 / RING_POINTS as f32;
            self.origin + (across * turn.cos() + beside * turn.sin()) * radius
        })
    }

    pub(crate) fn local(&self, world: Vec3) -> Vec3 {
        (self.turn.inverse() * world) / self.stretch.max(Vec3::splat(1e-6))
    }

    pub(crate) fn hit(&self, at: Point) -> Option<Part> {
        let centre = self.lens.project(self.origin)?;
        if centre.distance(at) <= FREE_REACH {
            return Some(Part::Free);
        }
        let plane = (0..3).find(|normal| {
            let corners = self.plane_corners(*normal);
            let middle = (corners[0] + corners[2]) * 0.5;
            self.lens
                .project(middle)
                .is_some_and(|middle| middle.distance(at) <= PLANE_SIDE * 0.75)
        });
        if let Some(normal) = plane {
            return Some(Part::Plane(normal));
        }
        let near = |points: &[Vec3]| -> f32 {
            let projected: Vec<Point> = points
                .iter()
                .filter_map(|point| self.lens.project(*point))
                .collect();
            projected
                .windows(2)
                .map(|pair| to_segment(at, pair[0], pair[1]))
                .fold(f32::INFINITY, f32::min)
        };
        let nearest = |parts: [(Part, f32); 3]| {
            parts
                .into_iter()
                .filter(|(_, away)| *away <= REACH)
                .min_by(|one, two| one.1.total_cmp(&two.1))
                .map(|(part, _)| part)
        };
        let axes = [0, 1, 2].map(|axis| {
            let start = self.origin + self.axes[axis] * (ARM * ARM_START * self.pixel);
            (Part::Axis(axis), near(&[start, self.arm(axis)]))
        });
        nearest(axes).or_else(|| {
            nearest([0, 1, 2].map(|axis| {
                let ring: Vec<Vec3> = self.ring(axis).collect();
                (Part::Ring(axis), near(&ring))
            }))
        })
    }
}

pub(crate) fn others(axis: usize) -> (usize, usize) {
    ((axis + 1) % 3, (axis + 2) % 3)
}

fn to_segment(at: Point, from: Point, to: Point) -> f32 {
    let along = to - from;
    let length = along.x * along.x + along.y * along.y;
    if length <= f32::EPSILON {
        return at.distance(from);
    }
    let share = (((at.x - from.x) * along.x + (at.y - from.y) * along.y) / length).clamp(0.0, 1.0);
    at.distance(from + along * share)
}
