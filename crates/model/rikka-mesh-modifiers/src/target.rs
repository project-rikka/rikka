use std::sync::{Arc, OnceLock};

use glam::Vec3;
use rikka_polymesh::{Polymesh, Triangulation};
use rikka_spatial::{PointTree, SegmentBvh, TriangleBvh};

use crate::error::Result;
use crate::kdop::KdopTree;
use crate::math::normalize;
use crate::normals::shading_point_normals;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BoundaryVertex {
    pub(crate) direction: Vec3,
    pub(crate) normal_plane: Vec3,
}

#[derive(Debug, Default)]
pub(crate) struct Boundary {
    pub(crate) edge_is_boundary: Vec<bool>,
    pub(crate) triangle_has_boundary: Vec<bool>,
    pub(crate) vertex_id: Vec<Option<u32>>,
    pub(crate) vertices: Vec<BoundaryVertex>,
}

impl Boundary {
    pub(crate) fn exists(&self) -> bool {
        !self.vertices.is_empty()
    }
}

#[derive(Debug, Default)]
struct Caches {
    triangulation: OnceLock<Triangulation>,
    triangles: OnceLock<TriangleBvh>,
    points: OnceLock<PointTree>,
    segments: OnceLock<SegmentBvh>,
    edge_tree: OnceLock<KdopTree>,
    face_normals: OnceLock<Vec<Vec3>>,
    point_normals: OnceLock<Vec<Vec3>>,
    corner_normals: OnceLock<Vec<Vec3>>,
    boundary: OnceLock<Boundary>,
}

#[derive(Debug)]
pub struct Target {
    mesh: Arc<Polymesh>,
    caches: Caches,
}

impl Target {
    pub fn new(mesh: Arc<Polymesh>) -> Result<Target> {
        mesh.validate()?;
        Ok(Target {
            mesh,
            caches: Caches::default(),
        })
    }

    #[must_use]
    pub fn mesh(&self) -> &Polymesh {
        &self.mesh
    }

    pub(crate) fn triangulation(&self) -> &Triangulation {
        self.caches
            .triangulation
            .get_or_init(|| self.mesh.triangulate())
    }

    pub(crate) fn triangle_points(&self, triangle: u32) -> [u32; 3] {
        self.triangulation().triangles[triangle as usize]
            .map(|corner| self.mesh.corner_verts[corner as usize])
    }

    pub(crate) fn triangle_positions(&self, triangle: u32) -> [Vec3; 3] {
        self.triangle_points(triangle)
            .map(|point| self.mesh.positions[point as usize])
    }

    pub(crate) fn triangles(&self) -> &TriangleBvh {
        self.caches.triangles.get_or_init(|| {
            let count = self.triangulation().triangles.len() as u32;
            let indices = (0..count)
                .map(|triangle| self.triangle_points(triangle))
                .collect();
            TriangleBvh::new(self.mesh.positions.clone(), indices)
                .expect("a validated mesh names only existing points")
        })
    }

    pub(crate) fn points(&self) -> &PointTree {
        self.caches
            .points
            .get_or_init(|| PointTree::new(self.mesh.positions.iter().copied()))
    }

    pub(crate) fn segments(&self) -> &SegmentBvh {
        self.caches.segments.get_or_init(|| {
            SegmentBvh::new(self.mesh.positions.clone(), self.mesh.edges.clone())
                .expect("a validated mesh names only existing points")
        })
    }

    pub(crate) fn edge_tree(&self) -> &KdopTree {
        self.caches.edge_tree.get_or_init(|| {
            KdopTree::new(self.mesh.edges.iter().enumerate().map(|(edge, &[a, b])| {
                (
                    edge as u32,
                    vec![
                        self.mesh.positions[a as usize],
                        self.mesh.positions[b as usize],
                    ],
                )
            }))
        })
    }

    pub(crate) fn face_normals(&self) -> &[Vec3] {
        self.caches
            .face_normals
            .get_or_init(|| self.mesh.face_normals())
    }

    pub(crate) fn point_normals(&self) -> &[Vec3] {
        self.caches
            .point_normals
            .get_or_init(|| shading_point_normals(&self.mesh))
    }

    pub(crate) fn corner_normals(&self) -> &[Vec3] {
        self.caches
            .corner_normals
            .get_or_init(|| self.mesh.corner_normals())
    }

    pub(crate) fn real_edges(&self, triangle: u32) -> [Option<u32>; 3] {
        let corners = self.triangulation().triangles[triangle as usize];
        let mesh = &self.mesh;
        std::array::from_fn(|index| {
            let (first, second) = (corners[index], corners[(index + 1) % 3]);
            let edge = mesh.corner_edges[first as usize];
            let [a, b] = mesh.edges[edge as usize];
            let (p, q) = (
                mesh.corner_verts[first as usize],
                mesh.corner_verts[second as usize],
            );
            ((p == a && q == b) || (p == b && q == a)).then_some(edge)
        })
    }

    pub(crate) fn boundary(&self) -> &Boundary {
        self.caches.boundary.get_or_init(|| self.build_boundary())
    }

    fn build_boundary(&self) -> Boundary {
        let mesh = &self.mesh;
        let mut users = vec![0u8; mesh.edge_count()];
        for &edge in &mesh.corner_edges {
            let count = &mut users[edge as usize];
            if *count < 2 {
                *count += 1;
            }
        }
        let edge_is_boundary: Vec<bool> = users.iter().map(|&count| count == 1).collect();
        if !edge_is_boundary.contains(&true) {
            return Boundary::default();
        }
        let triangle_count = self.triangulation().triangles.len() as u32;
        let triangle_has_boundary = (0..triangle_count)
            .map(|triangle| {
                self.real_edges(triangle)
                    .iter()
                    .flatten()
                    .any(|&edge| edge_is_boundary[edge as usize])
            })
            .collect();
        let mut tagged = vec![false; mesh.point_count()];
        for (edge, &boundary) in edge_is_boundary.iter().enumerate() {
            if boundary {
                let [a, b] = mesh.edges[edge];
                tagged[a as usize] = true;
                tagged[b as usize] = true;
            }
        }
        let mut count = 0u32;
        let vertex_id: Vec<Option<u32>> = tagged
            .iter()
            .map(|&tag| {
                tag.then(|| {
                    count += 1;
                    count - 1
                })
            })
            .collect();
        let mut vertices = vec![BoundaryVertex::default(); count as usize];
        let mut status = vec![0i8; count as usize];
        let mut merge = |id: u32, direction: Vec3, side: i8| {
            let id = id as usize;
            let current = &mut vertices[id].direction;
            let invert = match status[id] >= 0 {
                true => status[id] == side,
                false => current.dot(direction) < 0.0,
            };
            match invert {
                true => *current -= direction,
                false => *current += direction,
            }
            status[id] = match status[id] {
                0 => side,
                _ => -1,
            };
        };
        for (edge, &boundary) in edge_is_boundary.iter().enumerate() {
            if !boundary {
                continue;
            }
            let [a, b] = mesh.edges[edge];
            let direction = normalize(mesh.positions[b as usize] - mesh.positions[a as usize]);
            if let (Some(first), Some(second)) = (vertex_id[a as usize], vertex_id[b as usize]) {
                merge(first, direction, 1);
                merge(second, direction, 2);
            }
        }
        let normals = self.point_normals();
        for (point, id) in vertex_id.iter().enumerate() {
            let Some(id) = id else {
                continue;
            };
            let vertex = &mut vertices[*id as usize];
            vertex.direction = normalize(vertex.direction);
            let side = normals[point].cross(vertex.direction);
            vertex.normal_plane = normalize(side.cross(normals[point]));
        }
        Boundary {
            edge_is_boundary,
            triangle_has_boundary,
            vertex_id,
            vertices,
        }
    }
}
