use std::f32::consts::TAU;

use rikka_physics_math::{Plane, Transform, Vec3, cos_sin};

use super::blob::ConvexHull;
use crate::convex::queries;
use crate::{Error, Result};

impl ConvexHull {
    pub fn cuboid(half_extents: Vec3, transform: Transform) -> Result<Self> {
        for (what, value) in [
            ("hull half width", half_extents.x),
            ("hull half height", half_extents.y),
            ("hull half depth", half_extents.z),
        ] {
            queries::check_positive(what, value)?;
        }
        queries::check_transform("hull transform", transform)?;
        let corners = super::boxed::BOX_CORNERS
            .map(|corner| transform.apply(Vec3::from(corner) * half_extents));
        Self::new(&corners)
    }

    pub fn cylinder(height: f32, radius: f32, y_offset: f32, sides: usize) -> Result<Self> {
        queries::check_positive("hull cylinder height", height)?;
        queries::check_positive("hull cylinder radius", radius)?;
        let mut points = Vec::with_capacity(2 * sides);
        for (x, z) in ring(radius, sides) {
            points.push(Vec3::new(x, y_offset, z));
            points.push(Vec3::new(x, y_offset + height, z));
        }
        Self::new(&points)
    }

    pub fn cone(height: f32, radius1: f32, radius2: f32, slices: usize) -> Result<Self> {
        queries::check_positive("hull cone height", height)?;
        queries::check_positive("hull cone radius1", radius1)?;
        queries::check_positive("hull cone radius2", radius2)?;
        let mut points = Vec::with_capacity(2 * slices);
        for ((x1, z1), (x2, z2)) in ring(radius1, slices).zip(ring(radius2, slices)) {
            points.push(Vec3::new(x1, 0.0, z1));
            points.push(Vec3::new(x2, height, z2));
        }
        Self::new(&points)
    }

    pub fn rock(radius: f32) -> Result<Self> {
        queries::check_positive("hull rock radius", radius)?;
        const COUNT: usize = 10;
        let phi = 0.5 * (1.0 + 5.0f32.sqrt());
        let step = cos_sin(TAU / phi);
        let mut angle = (1.0f32, 0.0f32);
        let mut points = [Vec3::ZERO; COUNT];
        for (index, point) in points.iter_mut().enumerate() {
            let z = 1.0 - (2.0 * index as f32 + 1.0) / COUNT as f32;
            let planar = (1.0 - z * z).sqrt();
            *point = radius * Vec3::new(planar * angle.0, planar * angle.1, z);
            angle = (
                step.cosine * angle.0 - step.sine * angle.1,
                step.sine * angle.0 + step.cosine * angle.1,
            );
        }
        Self::new(&points)
    }

    pub fn transformed(&self, transform: Transform, scale: Vec3) -> Result<Self> {
        queries::check_transform("hull transform", transform)?;
        queries::check_point("hull scale", scale)?;
        let scale = scale.signum() * scale.abs().max(Vec3::splat(f32::EPSILON));
        let mut topology = self.topology();

        if scale.x * scale.y * scale.z < 0.0 {
            for &first in &topology.faces {
                let start = first as usize;
                let mut prev = start;
                while topology.edges[prev].next as usize != start {
                    prev = topology.edges[prev].next as usize;
                }
                let mut current = start;
                loop {
                    let next = topology.edges[current].next as usize;
                    topology.edges[current].next = prev as u16;
                    let twin = topology.edges[current].twin as usize;
                    if current < twin {
                        let origin = topology.edges[current].origin;
                        topology.edges[current].origin = topology.edges[twin].origin;
                        topology.edges[twin].origin = origin;
                    }
                    prev = current;
                    current = next;
                    if current == start {
                        break;
                    }
                }
            }
            for edge in &mut topology.vertex_edges {
                *edge = topology.edges[*edge as usize].twin;
            }
        }

        for point in &mut topology.points {
            *point = transform.apply(scale * *point);
        }

        for (face, &first) in topology.faces.iter().enumerate() {
            let mut count = 0;
            let mut centroid = Vec3::ZERO;
            let mut normal = Vec3::ZERO;
            let origin = topology.points[topology.edges[first as usize].origin as usize];
            let mut current = first as usize;
            loop {
                let edge = topology.edges[current];
                let v1 = topology.points[edge.origin as usize] - origin;
                let v2 =
                    topology.points[topology.edges[edge.twin as usize].origin as usize] - origin;
                count += 1;
                centroid += v1;
                normal.x += (v1.y - v2.y) * (v1.z + v2.z);
                normal.y += (v1.z - v2.z) * (v1.x + v2.x);
                normal.z += (v1.x - v2.x) * (v1.y + v2.y);
                current = edge.next as usize;
                if current == first as usize {
                    break;
                }
            }
            let centroid = centroid / count as f32 + origin;
            topology.planes[face] = Plane::from_normal_point(normal.normalize_or_zero(), centroid);
        }

        let count = topology.points.len();
        Self::from_topology(topology)
            .map(|hull| hull.rounded(self.radius()))
            .ok_or(Error::DegenerateHull { points: count })
    }
}

fn ring(radius: f32, sides: usize) -> impl Iterator<Item = (f32, f32)> {
    (0..sides).map(move |index| {
        let angle = cos_sin(TAU * index as f32 / sides as f32);
        (radius * angle.cosine, radius * angle.sine)
    })
}
