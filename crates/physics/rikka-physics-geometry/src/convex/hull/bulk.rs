use rikka_physics_math::{Mat3, Plane, Vec3};

use super::view::HullEdge;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bulk {
    pub(crate) center: Vec3,
    pub(crate) volume: f32,
    pub(crate) surface_area: f32,
    pub(crate) inner_radius: f32,
    pub(crate) inertia: Mat3,
}

pub(crate) fn bulk_properties(
    points: &[Vec3],
    edges: &[HullEdge],
    faces: &[u16],
    planes: &[Plane],
) -> Option<Bulk> {
    let origin = points[0];
    let mut area = 0.0f64;
    let mut volume = 0.0f64;
    let mut center = [0.0f64; 3];
    let mut products = [0.0f64; 6];

    for &first in faces {
        let i1 = first as usize;
        let mut i2 = edges[i1].next as usize;
        let mut i3 = edges[i2].next as usize;
        let v1 = (points[edges[i1].origin as usize] - origin).as_dvec3();
        while i3 != i1 {
            let v2 = (points[edges[i2].origin as usize] - origin).as_dvec3();
            let v3 = (points[edges[i3].origin as usize] - origin).as_dvec3();
            area += (v2 - v1).cross(v3 - v1).length();
            let det = v1.dot(v2.cross(v3));
            volume += det;
            let v4 = v1 + v2 + v3;
            for axis in 0..3 {
                center[axis] += det * v4[axis];
            }
            let square =
                |i: usize, j: usize| v1[i] * v1[j] + v2[i] * v2[j] + v3[i] * v3[j] + v4[i] * v4[j];
            products[0] += det * square(0, 0);
            products[1] += det * square(1, 1);
            products[2] += det * square(2, 2);
            products[3] += det * square(0, 1);
            products[4] += det * square(0, 2);
            products[5] += det * square(1, 2);
            i2 = i3;
            i3 = edges[i3].next as usize;
        }
    }

    if volume <= 0.0 || area <= 0.0 {
        return None;
    }

    let local_center =
        Vec3::new(center[0] as f32, center[1] as f32, center[2] as f32) * (0.25 / volume) as f32;
    let world_center = local_center + origin;

    let mut inner_radius = f32::MAX;
    for plane in planes {
        let distance = plane.separation(world_center);
        if distance >= 0.0 {
            return None;
        }
        inner_radius = inner_radius.min(-distance);
    }

    let [xx, yy, zz, xy, xz, yz] = products.map(|value| value / 120.0);
    let mass = volume / 6.0;
    let c = local_center.as_dvec3();
    let steiner = |i: usize, j: usize| {
        let diagonal = if i == j { c.length_squared() } else { 0.0 };
        mass * (diagonal - c[i] * c[j])
    };
    let inertia = [
        [
            yy + zz - steiner(0, 0),
            -xy - steiner(0, 1),
            -xz - steiner(0, 2),
        ],
        [
            -xy - steiner(1, 0),
            xx + zz - steiner(1, 1),
            -yz - steiner(1, 2),
        ],
        [
            -xz - steiner(2, 0),
            -yz - steiner(2, 1),
            xx + yy - steiner(2, 2),
        ],
    ];
    let column = |j: usize| {
        Vec3::new(
            inertia[0][j] as f32,
            inertia[1][j] as f32,
            inertia[2][j] as f32,
        )
    };

    Some(Bulk {
        center: world_center,
        volume: mass as f32,
        surface_area: (0.5 * area) as f32,
        inner_radius,
        inertia: Mat3::from_cols(column(0), column(1), column(2)),
    })
}
