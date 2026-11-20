use rikka_physics_math::{Mat3, Plane, Transform, Vec3};

use super::view::{EDGE_ROWS, HullEdge, Polyhedron};

pub(crate) const BOX_CORNERS: [[f32; 3]; 8] = [
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
];

pub(crate) const BOX_EDGES: [HullEdge; 24] = [
    HullEdge::new(2, 1, 0, 2),
    HullEdge::new(17, 0, 5, 1),
    HullEdge::new(4, 3, 0, 1),
    HullEdge::new(20, 2, 3, 5),
    HullEdge::new(6, 5, 0, 5),
    HullEdge::new(23, 4, 4, 6),
    HullEdge::new(0, 7, 0, 6),
    HullEdge::new(18, 6, 2, 2),
    HullEdge::new(10, 9, 1, 0),
    HullEdge::new(21, 8, 5, 3),
    HullEdge::new(12, 11, 1, 3),
    HullEdge::new(16, 10, 2, 7),
    HullEdge::new(14, 13, 1, 7),
    HullEdge::new(19, 12, 4, 4),
    HullEdge::new(8, 15, 1, 4),
    HullEdge::new(22, 14, 3, 0),
    HullEdge::new(7, 17, 2, 3),
    HullEdge::new(9, 16, 5, 2),
    HullEdge::new(11, 19, 2, 6),
    HullEdge::new(5, 18, 4, 7),
    HullEdge::new(15, 21, 3, 1),
    HullEdge::new(1, 20, 5, 0),
    HullEdge::new(3, 23, 3, 4),
    HullEdge::new(13, 22, 4, 5),
];

pub(crate) const BOX_PAIR_FACES: [[usize; 2]; 12] = {
    let mut out = [[0; 2]; 12];
    let mut pair = 0;
    while pair < 12 {
        out[pair] = [
            BOX_EDGES[2 * pair].face as usize,
            BOX_EDGES[2 * pair + 1].face as usize,
        ];
        pair += 1;
    }
    out
};
pub(crate) const BOX_FACES: [u16; 6] = [0, 8, 16, 20, 19, 21];

pub(crate) const BOX_VERTEX_EDGES: [u16; 8] = [8, 1, 0, 9, 13, 3, 5, 11];

#[inline]
pub(crate) fn corner_index(signs: Vec3) -> u32 {
    let low_x = u32::from(signs.x < 0.0);
    let low_y = u32::from(signs.y < 0.0);
    let low_z = u32::from(signs.z < 0.0);
    2 * low_y + (low_x ^ low_y) + 4 * low_z
}

const BOX_EDGE_STRIDE: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxPolyhedron {
    points: [Vec3; 8],
    planes: [Plane; 6],
    soa_points: [f32; 24],
    soa_normals: [f32; 24],
    edge_normals: [f32; EDGE_ROWS * BOX_EDGE_STRIDE],
    face_distances: [f32; 8],
    center: Vec3,
    inner_radius: f32,
    axes: [Vec3; 3],
}

impl BoxPolyhedron {
    pub(crate) fn new(transform: Transform, half_extents: Vec3) -> Self {
        let rotation = Mat3::from_quat(transform.q);
        let columns = [rotation.x_axis, rotation.y_axis, rotation.z_axis];
        let axes = [
            columns[0] * half_extents.x,
            columns[1] * half_extents.y,
            columns[2] * half_extents.z,
        ];
        let points =
            BOX_CORNERS.map(|[x, y, z]| transform.p + x * axes[0] + y * axes[1] + z * axes[2]);
        let reach = half_extents.to_array();
        let planes: [Plane; 6] = std::array::from_fn(|face| {
            let normal = if face % 2 == 0 {
                -columns[face / 2]
            } else {
                columns[face / 2]
            };
            Plane::new(normal, normal.dot(transform.p) + reach[face / 2])
        });
        let mut soa_points = [0.0; 24];
        let mut soa_normals = [0.0; 24];
        for axis in 0..3 {
            for (index, point) in points.iter().enumerate() {
                soa_points[8 * axis + index] = point[axis];
            }
            for (index, plane) in planes.iter().enumerate() {
                soa_normals[8 * axis + index] = plane.normal[axis];
            }
        }
        let mut edge_normals = [0.0; EDGE_ROWS * BOX_EDGE_STRIDE];
        for (pair, [first, second]) in BOX_PAIR_FACES.into_iter().enumerate() {
            for axis in 0..3 {
                edge_normals[axis * BOX_EDGE_STRIDE + pair] = planes[first].normal[axis];
                edge_normals[(axis + 3) * BOX_EDGE_STRIDE + pair] = planes[second].normal[axis];
            }
            edge_normals[7 * BOX_EDGE_STRIDE + pair] = reach[first / 2].min(reach[second / 2]);
        }
        Self {
            points,
            planes,
            soa_points,
            soa_normals,
            edge_normals,
            face_distances: [
                half_extents.x,
                half_extents.x,
                half_extents.y,
                half_extents.y,
                half_extents.z,
                half_extents.z,
                0.0,
                0.0,
            ],
            center: transform.p,
            inner_radius: half_extents.min_element(),
            axes,
        }
    }

    #[inline]
    pub(crate) fn view(&self) -> Polyhedron<'_> {
        Polyhedron {
            points: &self.points,
            vertex_edges: &BOX_VERTEX_EDGES,
            edges: &BOX_EDGES,
            faces: &BOX_FACES,
            planes: &self.planes,
            soa_points: &self.soa_points,
            soa_normals: &self.soa_normals,
            edge_normals: &self.edge_normals,
            face_distances: &self.face_distances,
            center: self.center,
            inner_radius: self.inner_radius,
            box_axes: self.axes,
        }
    }
}
