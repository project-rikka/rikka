use std::sync::OnceLock;

use rikka_physics_math::{Plane, Transform, Vec3};

use crate::{ConvexHull, Cuboid, Cylinder, HullEdge};

const CYLINDER_SIDES: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct HullView<'a> {
    pub(crate) points: &'a [Vec3],
    pub(crate) planes: &'a [Plane],
    pub(crate) edges: &'a [HullEdge],
    pub(crate) faces: &'a [u16],
    pub(crate) vertex_edges: &'a [u16],
    pub(crate) soa_points: Option<&'a [f32]>,
    pub(crate) soa_normals: Option<&'a [f32]>,
    pub(crate) edge_normals: Option<&'a [f32]>,
    pub(crate) center: Vec3,
    pub(crate) radius: f32,
}

#[derive(Debug, Default)]
pub(crate) struct HullStorage {
    points: Vec<Vec3>,
    planes: Vec<Plane>,
}

struct Topology {
    hull: ConvexHull,
}

impl Topology {
    fn edges(&self) -> &[HullEdge] {
        self.hull.edges()
    }

    fn faces(&self) -> &[u16] {
        self.hull.blob().section(self.hull.header().faces)
    }

    fn vertex_edges(&self) -> &[u16] {
        self.hull.blob().section(self.hull.header().vertex_edges)
    }
}

fn unit_cube() -> &'static Topology {
    static CUBE: OnceLock<Topology> = OnceLock::new();
    CUBE.get_or_init(|| Topology {
        hull: ConvexHull::cuboid(Vec3::ONE, Transform::IDENTITY).expect("unit cube hull is valid"),
    })
}

fn unit_cylinder() -> &'static Topology {
    static CYLINDER: OnceLock<Topology> = OnceLock::new();
    CYLINDER.get_or_init(|| Topology {
        hull: ConvexHull::cylinder(2.0, 1.0, -1.0, CYLINDER_SIDES)
            .expect("unit cylinder hull is valid"),
    })
}

impl<'a> HullView<'a> {
    pub(crate) fn of_hull(hull: &'a ConvexHull) -> Self {
        let poly = hull.polyhedron();
        Self {
            points: poly.points,
            planes: poly.planes,
            edges: poly.edges,
            faces: poly.faces,
            vertex_edges: poly.vertex_edges,
            soa_points: Some(poly.soa_points),
            soa_normals: Some(poly.soa_normals),
            edge_normals: Some(poly.edge_normals),
            center: poly.center,
            radius: hull.radius(),
        }
    }

    pub(crate) fn of_cuboid(cuboid: &Cuboid, storage: &'a mut HullStorage) -> Self {
        Self::scaled(
            unit_cube(),
            cuboid.transform,
            cuboid.half_extents,
            cuboid.radius,
            storage,
        )
    }

    pub(crate) fn of_cylinder(cylinder: &Cylinder, storage: &'a mut HullStorage) -> Self {
        let scale = Vec3::new(cylinder.radius, cylinder.half_height, cylinder.radius);
        Self::scaled(unit_cylinder(), cylinder.transform, scale, 0.0, storage)
    }

    fn scaled(
        topology: &'static Topology,
        transform: Transform,
        scale: Vec3,
        radius: f32,
        storage: &'a mut HullStorage,
    ) -> Self {
        let hull = &topology.hull;
        storage.points.clear();
        storage.points.extend(
            hull.points()
                .iter()
                .map(|&point| transform.apply(point * scale)),
        );
        storage.planes.clear();
        let faces = topology.faces();
        storage
            .planes
            .extend(hull.planes().iter().zip(faces).map(|(plane, &edge)| {
                let local = (plane.normal / scale).normalize();
                let origin = hull.points()[topology.edges()[edge as usize].origin as usize] * scale;
                let normal = transform.rotate(local);
                Plane::new(normal, local.dot(origin) + normal.dot(transform.p))
            }));
        Self {
            points: &storage.points,
            planes: &storage.planes,
            edges: topology.edges(),
            faces,
            vertex_edges: topology.vertex_edges(),
            soa_points: None,
            soa_normals: None,
            edge_normals: None,
            center: transform.p,
            radius,
        }
    }

    #[inline]
    pub(crate) fn edge_points(&self, edge: usize) -> (Vec3, Vec3) {
        let half = self.edges[edge];
        let twin = self.edges[half.twin as usize];
        (
            self.points[half.origin as usize],
            self.points[twin.origin as usize],
        )
    }

    pub(crate) fn face_edges(&self, face: usize) -> impl Iterator<Item = usize> + '_ {
        let first = self.faces[face] as usize;
        let mut current = Some(first);
        std::iter::from_fn(move || {
            let edge = current?;
            let next = self.edges[edge].next as usize;
            current = (next != first).then_some(next);
            Some(edge)
        })
    }

    pub(crate) fn incident_face(&self, reference: Vec3, vertex: usize) -> usize {
        let start = self.vertex_edges[vertex] as usize;
        let origin = self.points[vertex];
        let mut edge = start;
        let mut best_edge = start;
        let mut best_projection = f32::MAX;
        loop {
            let twin = self.edges[edge].twin as usize;
            let axis = (self.points[self.edges[twin].origin as usize] - origin).normalize_or_zero();
            let projection = axis.dot(reference).abs();
            if projection < best_projection {
                best_edge = edge;
                best_projection = projection;
            }
            edge = self.edges[twin].next as usize;
            if edge == start {
                break;
            }
        }
        let face1 = self.edges[best_edge].face as usize;
        let face2 = self.edges[self.edges[best_edge].twin as usize].face as usize;
        if self.planes[face1].normal.dot(reference) < self.planes[face2].normal.dot(reference) {
            face1
        } else {
            face2
        }
    }
}
