use std::f32::consts::GOLDEN_RATIO;
use std::sync::OnceLock;

use glam::Vec3;

use super::pose::Pose;
use crate::asset::BodyShape;
use crate::geometry::Principal;

const SAME_CORNER: f32 = 1e-12;

#[derive(Clone, Debug)]
pub(crate) struct Hull {
    normals: Vec<Vec3>,
    offsets: Vec<f32>,
    corners: Vec<Vec3>,
}

impl Hull {
    pub(crate) fn fit(points: &[Vec3], frame: &Principal) -> Hull {
        let normals: Vec<Vec3> = directions()
            .iter()
            .map(|d| (frame.axes[0] * d.x + frame.axes[1] * d.y + frame.axes[2] * d.z).normalize())
            .collect();
        let offsets = normals
            .iter()
            .map(|normal| {
                points
                    .iter()
                    .map(|point| normal.dot(*point))
                    .fold(f32::MIN, f32::max)
            })
            .collect();
        Hull::bounded(normals, offsets)
    }

    fn bounded(normals: Vec<Vec3>, offsets: Vec<f32>) -> Hull {
        let corners = corners(&normals, &offsets);
        Hull {
            normals,
            offsets,
            corners,
        }
    }

    pub(crate) fn shape(&self) -> BodyShape {
        BodyShape::Hull {
            planes: self
                .normals
                .iter()
                .copied()
                .zip(self.offsets.iter().copied())
                .collect(),
        }
    }

    pub(crate) fn is_solid(&self) -> bool {
        self.corners.len() >= 4 && self.thinnest() > 0.0
    }

    pub(crate) fn distance(&self, point: Vec3) -> f32 {
        self.normals
            .iter()
            .zip(&self.offsets)
            .map(|(normal, offset)| normal.dot(point) - offset)
            .fold(f32::MIN, f32::max)
    }

    pub(crate) fn shrunk(&self, by: f32) -> Hull {
        Hull::bounded(
            self.normals.clone(),
            self.offsets.iter().map(|offset| offset - by).collect(),
        )
    }

    pub(crate) fn thinnest(&self) -> f32 {
        self.widths().fold(f32::MAX, f32::min)
    }

    pub(crate) fn moved(&self, pose: Pose) -> Hull {
        let normals: Vec<Vec3> = self
            .normals
            .iter()
            .map(|normal| pose.rotate(*normal))
            .collect();
        let offsets = normals
            .iter()
            .zip(&self.offsets)
            .map(|(normal, offset)| offset + normal.dot(pose.translation))
            .collect();
        let corners = self
            .corners
            .iter()
            .map(|corner| pose.apply(*corner))
            .collect();
        Hull {
            normals,
            offsets,
            corners,
        }
    }

    pub(crate) fn bounds(&self) -> (Vec3, Vec3) {
        self.corners.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), corner| (low.min(*corner), high.max(*corner)),
        )
    }

    fn widths(&self) -> impl Iterator<Item = f32> + '_ {
        let half = self.offsets.len() / 2;
        (0..half).map(move |index| (self.offsets[index] + self.offsets[index + half]) * 0.5)
    }
}

fn corners(normals: &[Vec3], offsets: &[f32]) -> Vec<Vec3> {
    let reach = offsets
        .iter()
        .fold(1.0f32, |largest, offset| largest.max(offset.abs()))
        * 4.0;
    let mut corners: Vec<Vec3> = Vec::new();
    for (face, (normal, offset)) in normals.iter().zip(offsets).enumerate() {
        let (u, v) = normal.any_orthonormal_pair();
        let centre = *normal * *offset;
        let mut polygon: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .map(|(a, b)| centre + (u * a + v * b) * reach)
            .collect();
        for (other, (cut, limit)) in normals.iter().zip(offsets).enumerate() {
            if other != face && !polygon.is_empty() {
                polygon = clipped(&polygon, *cut, *limit);
            }
        }
        for corner in polygon {
            if !corners
                .iter()
                .any(|clipped| clipped.distance_squared(corner) <= SAME_CORNER)
            {
                corners.push(corner);
            }
        }
    }
    corners
}

fn clipped(polygon: &[Vec3], normal: Vec3, offset: f32) -> Vec<Vec3> {
    let mut clipped = Vec::with_capacity(polygon.len() + 1);
    for (index, &from) in polygon.iter().enumerate() {
        let to = polygon[(index + 1) % polygon.len()];
        let (a, b) = (normal.dot(from) - offset, normal.dot(to) - offset);
        if a <= 0.0 {
            clipped.push(from);
        }
        if (a <= 0.0) != (b <= 0.0) {
            clipped.push(from.lerp(to, a / (a - b)));
        }
    }
    clipped
}

fn directions() -> &'static [Vec3] {
    static DIRECTIONS: OnceLock<Vec<Vec3>> = OnceLock::new();
    DIRECTIONS.get_or_init(|| {
        let mut corners = Vec::with_capacity(12);
        for a in [-1.0, 1.0] {
            for b in [-GOLDEN_RATIO, GOLDEN_RATIO] {
                corners.push(Vec3::new(0.0, a, b));
                corners.push(Vec3::new(a, b, 0.0));
                corners.push(Vec3::new(b, 0.0, a));
            }
        }
        let mut all: Vec<Vec3> = corners.iter().map(|corner| corner.normalize()).collect();
        for (index, a) in corners.iter().enumerate() {
            for b in &corners[index + 1..] {
                if (a.distance(*b) - 2.0).abs() < 1e-3 {
                    all.push(((*a + *b) * 0.5).normalize());
                }
            }
        }
        let upper = |v: &&Vec3| {
            let lead = [v.x, v.y, v.z]
                .into_iter()
                .find(|c| c.abs() > 1e-6)
                .unwrap_or(0.0);
            lead > 0.0
        };
        let half: Vec<Vec3> = all.iter().filter(upper).copied().collect();
        half.iter()
            .copied()
            .chain(half.iter().map(|v| -*v))
            .collect()
    })
}
