use glam::{Mat3, Vec2, Vec3};

use crate::asset::{BindTarget, Influence, Piece, StrandBinding, StrandNode, StrandWeight};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub tangent: Vec3,
    pub bitangent: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrapedVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
}

impl Frame {
    #[must_use]
    pub fn from_triangle(corners: [Vec3; 3], normals: [Vec3; 3], uv: Vec2) -> Frame {
        let [v0, v1, v2] = corners;
        let rest = 1.0 - uv.x - uv.y;
        let origin = v0 * rest + v1 * uv.x + v2 * uv.y;
        let blended = normals[0] * rest + normals[1] * uv.x + normals[2] * uv.y;
        let face = (v1 - v0).cross(v2 - v0).normalize_or(Vec3::Y);
        let normal = blended.normalize_or(face);
        let edge = v1 - v0;
        let tangent = (edge - normal * normal.dot(edge))
            .try_normalize()
            .unwrap_or_else(|| {
                ((v2 - v0) - normal * normal.dot(v2 - v0))
                    .normalize_or(normal.any_orthonormal_vector())
            });
        Frame {
            origin,
            tangent,
            bitangent: normal.cross(tangent),
            normal,
        }
    }

    #[must_use]
    pub fn from_axes(origin: Vec3, along: Vec3, side: Vec3) -> Frame {
        let tangent = along.normalize_or(Vec3::NEG_Y);
        let bitangent =
            (side - tangent * tangent.dot(side)).normalize_or(tangent.any_orthonormal_vector());
        Frame {
            origin,
            tangent,
            bitangent,
            normal: tangent.cross(bitangent),
        }
    }

    #[must_use]
    pub fn basis(&self) -> Mat3 {
        Mat3::from_cols(self.tangent, self.bitangent, self.normal)
    }
    #[must_use]
    pub fn point(&self, offset: Vec3) -> Vec3 {
        self.origin + self.vector(offset)
    }

    #[must_use]
    pub fn vector(&self, local: Vec3) -> Vec3 {
        self.tangent * local.x + self.bitangent * local.y + self.normal * local.z
    }

    #[must_use]
    pub fn local_point(&self, point: Vec3) -> Vec3 {
        self.local_vector(point - self.origin)
    }

    #[must_use]
    pub fn local_vector(&self, vector: Vec3) -> Vec3 {
        Vec3::new(
            vector.dot(self.tangent),
            vector.dot(self.bitangent),
            vector.dot(self.normal),
        )
    }
}

impl StrandNode {
    #[must_use]
    pub fn frame(&self, positions: &[Vec3]) -> Frame {
        let origin = positions[self.origin as usize];
        let [from, to] = self.along.map(|particle| positions[particle as usize]);
        Frame::from_axes(origin, to - from, positions[self.side as usize] - origin)
    }

    #[must_use]
    pub fn rotation(&self, rest: &[Vec3], positions: &[Vec3]) -> Mat3 {
        self.frame(positions).basis() * self.frame(rest).basis().transpose()
    }
}

impl StrandBinding {
    #[must_use]
    pub fn origin(&self, strand_nodes: &[StrandNode], positions: &[Vec3]) -> Vec3 {
        let sum: Vec3 = self
            .active_weights()
            .map(|entry| {
                positions[strand_nodes[entry.strand_node as usize].origin as usize] * entry.weight
            })
            .sum();
        sum / self.total()
    }

    #[must_use]
    pub fn frame(&self, strand_nodes: &[StrandNode], rest: &[Vec3], positions: &[Vec3]) -> Frame {
        let rotation = self
            .active_weights()
            .map(|entry| {
                strand_nodes[entry.strand_node as usize].rotation(rest, positions) * entry.weight
            })
            .fold(Mat3::ZERO, |sum, rotation| sum + rotation);
        Frame::from_axes(
            self.origin(strand_nodes, positions),
            rotation.x_axis,
            rotation.y_axis,
        )
    }

    fn active_weights(&self) -> impl Iterator<Item = &StrandWeight> {
        self.weights.iter().filter(|entry| entry.weight > 0.0)
    }

    fn total(&self) -> f32 {
        self.active_weights()
            .map(|entry| entry.weight)
            .sum::<f32>()
            .max(f32::MIN_POSITIVE)
    }
}

#[must_use]
pub fn particle_normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut sums = vec![Vec3::ZERO; positions.len()];
    for triangle in triangles {
        let [a, b, c] = triangle.map(|corner| positions[corner as usize]);
        let weighted = (b - a).cross(c - a);
        for corner in triangle {
            sums[*corner as usize] += weighted;
        }
    }
    sums.into_iter().map(Vec3::normalize_or_zero).collect()
}

impl Piece {
    #[must_use]
    pub fn drape(&self, positions: &[Vec3]) -> Vec<DrapedVertex> {
        let normals = particle_normals(positions, &self.triangles);
        let rest = self.rest_positions();
        self.bindings
            .iter()
            .map(|bind| match &bind.target {
                BindTarget::Triangles(influences) => {
                    on_triangles(influences, positions, &normals, &self.triangles)
                }
                BindTarget::Strand(strand) => {
                    let frame = strand.frame(&self.strand_nodes, &rest, positions);
                    DrapedVertex {
                        position: frame.point(strand.offset),
                        normal: frame.vector(strand.normal).normalize_or_zero(),
                        tangent: frame.vector(strand.tangent).normalize_or_zero(),
                    }
                }
            })
            .collect()
    }

    #[must_use]
    pub fn rest_positions(&self) -> Vec<Vec3> {
        self.particles
            .iter()
            .map(|particle| particle.rest)
            .collect()
    }
}

fn on_triangles(
    influences: &[Influence],
    positions: &[Vec3],
    normals: &[Vec3],
    triangles: &[[u32; 3]],
) -> DrapedVertex {
    let mut draped = DrapedVertex {
        position: Vec3::ZERO,
        normal: Vec3::ZERO,
        tangent: Vec3::ZERO,
    };
    for influence in influences.iter().filter(|influence| influence.weight > 0.0) {
        let triangle = triangles[influence.triangle as usize];
        let frame = Frame::from_triangle(
            triangle.map(|corner| positions[corner as usize]),
            triangle.map(|corner| normals[corner as usize]),
            influence.uv,
        );
        draped.position += frame.point(influence.offset) * influence.weight;
        draped.normal += frame.vector(influence.normal) * influence.weight;
        draped.tangent += frame.vector(influence.tangent) * influence.weight;
    }
    draped.normal = draped.normal.normalize_or_zero();
    draped.tangent = draped.tangent.normalize_or_zero();
    draped
}
