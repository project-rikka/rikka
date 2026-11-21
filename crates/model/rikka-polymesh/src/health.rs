use std::collections::VecDeque;

use glam::Vec3;
use rikka_spatial::TriangleBvh;

use crate::polymesh::Polymesh;
use crate::topology::{Components, EdgeKind};

const DEGENERATE_AREA_RATIO: f32 = 1e-10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Orientation {
    pub components: Components,
    pub closed: Vec<bool>,
    pub inconsistent_edges: Vec<bool>,
    pub flipped: Vec<bool>,
}

impl Polymesh {
    fn vector_area(&self, face: usize) -> Vec3 {
        let verts = self.face_verts(face);
        let origin = self.positions[verts[0] as usize];
        let mut normal = Vec3::ZERO;
        for pair in verts[1..].windows(2) {
            normal += (self.positions[pair[0] as usize] - origin)
                .cross(self.positions[pair[1] as usize] - origin);
        }
        normal * 0.5
    }

    fn corner_direction(&self, face: usize, corner: usize) -> (u32, u32) {
        (
            self.corner_verts[corner],
            self.corner_verts[self.next_corner(face, corner)],
        )
    }

    #[must_use]
    pub fn orientation(&self) -> Orientation {
        let faces = self.face_count();
        let kinds = self.edge_kinds();
        let owners = self.corner_faces();
        let edge_corners = self.edge_corners();
        let inconsistent_edges: Vec<bool> = (0..self.edge_count())
            .map(|edge| {
                let corners = edge_corners.get(edge);
                kinds[edge] == EdgeKind::Manifold && {
                    let (first, second) = (corners[0] as usize, corners[1] as usize);
                    self.corner_direction(owners[first] as usize, first)
                        == self.corner_direction(owners[second] as usize, second)
                }
            })
            .collect();
        let mut reversed: Vec<Option<bool>> = vec![None; faces];
        let mut ids = vec![0u32; faces];
        let mut closed = Vec::new();
        let mut flipped = vec![false; faces];
        let mut queue = VecDeque::new();
        for seed in 0..faces {
            if reversed[seed].is_some() {
                continue;
            }
            let component = closed.len() as u32;
            reversed[seed] = Some(false);
            queue.push_back(seed);
            let mut members = vec![seed];
            let mut sealed = true;
            while let Some(face) = queue.pop_front() {
                ids[face] = component;
                let own = reversed[face].unwrap_or(false);
                for corner in self.face(face) {
                    let edge = self.corner_edges[corner] as usize;
                    if kinds[edge] != EdgeKind::Manifold {
                        sealed = false;
                        continue;
                    }
                    let corners = edge_corners.get(edge);
                    let other = match corners[0] as usize == corner {
                        true => corners[1] as usize,
                        false => corners[0] as usize,
                    };
                    let neighbour = owners[other] as usize;
                    if reversed[neighbour].is_none() {
                        reversed[neighbour] = Some(own ^ inconsistent_edges[edge]);
                        members.push(neighbour);
                        queue.push_back(neighbour);
                    }
                }
            }
            let (mut kept, mut turned, mut volume) = (0.0f64, 0.0f64, 0.0f64);
            for &face in &members {
                let area = self.vector_area(face);
                let first = self.positions[self.face_verts(face)[0] as usize];
                let length = f64::from(area.length());
                let signed = f64::from(first.dot(area));
                match reversed[face] == Some(true) {
                    true => {
                        turned += length;
                        volume -= signed;
                    }
                    false => {
                        kept += length;
                        volume += signed;
                    }
                }
            }
            let outward_reversed = match sealed {
                true => volume < 0.0,
                false => kept < turned,
            };
            for &face in &members {
                flipped[face] = reversed[face] != Some(outward_reversed);
            }
            closed.push(sealed);
        }
        Orientation {
            components: Components {
                ids,
                count: closed.len(),
            },
            closed,
            inconsistent_edges,
            flipped,
        }
    }

    #[must_use]
    pub fn degenerate_faces(&self) -> Vec<bool> {
        let (low, high) = self.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), &point| (low.min(point), high.max(point)),
        );
        let diagonal = match self.positions.is_empty() {
            true => 0.0,
            false => low.distance(high),
        };
        let limit = diagonal * diagonal * DEGENERATE_AREA_RATIO;
        (0..self.face_count())
            .map(|face| self.vector_area(face).length() <= limit)
            .collect()
    }

    #[must_use]
    pub fn self_intersections(&self) -> Vec<[u32; 2]> {
        let triangulation = self.point_triangulation();
        let mut pairs = Vec::new();
        if let Ok(bvh) = TriangleBvh::new(self.positions.clone(), triangulation.triangles) {
            bvh.for_each_self_intersection(|pair| {
                pairs.push([
                    triangulation.faces[pair.first as usize],
                    triangulation.faces[pair.second as usize],
                ]);
            });
        }
        pairs
    }
}
