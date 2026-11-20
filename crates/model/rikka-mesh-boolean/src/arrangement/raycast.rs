use glam::{DVec3, Vec3};
use rikka_spatial::{Culling, Ray, TriangleBvh};

use crate::arrangement::cells::Patches;
use crate::arrangement::{Context, Triangle};
use crate::operation::Operation;
use crate::parallel::map_chunks;

const OFFSET: f64 = 1.0e-5;
const RAYS: [[f32; 3]; 6] = {
    const R1: f32 = 0.998_702_5;
    const RA: f32 = 0.049_935_126;
    const RB: f32 = 0.009_987_025;
    [
        [R1, RA, RB],
        [-R1, -RA, -RB],
        [RB, R1, RA],
        [-RB, -R1, -RA],
        [RA, RB, R1],
        [-RA, -RB, -R1],
    ]
};
const TRIANGLES_PER_TASK: usize = 256;

struct Scene<'a> {
    context: &'a Context<'a>,
    triangles: &'a [Triangle],
    corners: Vec<[DVec3; 3]>,
    bvh: Option<TriangleBvh>,
}

impl Scene<'_> {
    fn inside_shapes(&self, test: usize) -> Vec<f32> {
        let shapes = self.context.shapes;
        let own = self.context.shape(&self.triangles[test]) as usize;
        let mut result = vec![0.0; shapes];
        let [a, b, c] = self.corners[test];
        let center = a / 3.0 + b / 3.0 + c / 3.0;
        let normal = (b - a).cross(c - a).normalize_or_zero();
        let origin = (center + normal * OFFSET).as_vec3();
        let mut insides = vec![0usize; shapes];
        if let Some(bvh) = &self.bvh {
            for direction in RAYS {
                let Some(ray) = Ray::new(origin, Vec3::from_array(direction)) else {
                    continue;
                };
                let mut parity = vec![0i32; shapes];
                for hit in bvh.ray_cast_all(&ray, f32::MAX, Culling::None) {
                    let hit_triangle = &self.triangles[hit.triangle as usize];
                    let shape = self.context.shape(hit_triangle) as usize;
                    let [p, q, r] = self.corners[hit.triangle as usize];
                    let side = (q - p).cross(r - p).dot(origin.as_dvec3() - p);
                    parity[shape] += match side {
                        s if s < 0.0 => 1,
                        s if s > 0.0 => -1,
                        _ => 0,
                    };
                }
                for (shape, &value) in parity.iter().enumerate() {
                    if shape != own && value > 0 {
                        insides[shape] += 1;
                    }
                }
            }
        }
        for shape in 0..shapes {
            result[shape] = match shape == own {
                true => 1.0,
                false => insides[shape] as f32 / RAYS.len() as f32,
            };
        }
        result
    }

    fn decide(&self, test: usize, operation: Operation) -> (bool, bool) {
        let own = self.context.shape(&self.triangles[test]) as usize;
        let inside = self.inside_shapes(test);
        let high_confidence =
            (operation == Operation::Difference && own != 0) || operation == Operation::Intersect;
        let threshold = if high_confidence { 0.5 } else { 0.1 };
        let mut winding: Vec<i32> = inside
            .iter()
            .map(|&value| i32::from(value >= threshold))
            .collect();
        winding[own] = 0;
        let outside = operation.keeps(&winding);
        winding[own] = 1;
        let inside_own = operation.keeps(&winding);
        let remove = outside == inside_own;
        let flip = !remove && operation == Operation::Difference && own != 0;
        (remove, flip)
    }
}

pub(crate) fn classify(
    context: &Context,
    triangles: &[Triangle],
    operation: Operation,
    patches: Option<&Patches>,
) -> Vec<Triangle> {
    let corners: Vec<[DVec3; 3]> = triangles
        .iter()
        .map(|triangle| triangle.verts.map(|v| context.point(v).approx))
        .collect();
    let bvh = TriangleBvh::from_triangles(corners.iter().map(|tri| tri.map(DVec3::as_vec3))).ok();
    let scene = Scene {
        context,
        triangles,
        corners,
        bvh,
    };
    let threads = context.settings.threads;
    match patches {
        None => {
            let decisions: Vec<(bool, bool)> =
                map_chunks(triangles.len(), threads, TRIANGLES_PER_TASK, |range| {
                    range.map(|test| scene.decide(test, operation)).collect()
                });
            triangles
                .iter()
                .zip(decisions)
                .filter(|(_, (remove, _))| !remove)
                .map(|(triangle, (_, flip))| match flip {
                    true => triangle.flip(),
                    false => *triangle,
                })
                .collect()
        }
        Some(patches) => {
            let decisions: Vec<(bool, bool)> =
                map_chunks(patches.list.len(), threads, 1, |range| {
                    range
                        .map(|patch| {
                            let tris = &patches.list[patch].tris;
                            scene.decide(tris[tris.len() / 2] as usize, operation)
                        })
                        .collect()
                });
            let mut output = Vec::with_capacity(triangles.len());
            for (patch, (remove, flip)) in patches.list.iter().zip(decisions) {
                if remove {
                    continue;
                }
                for &tri in &patch.tris {
                    let triangle = &triangles[tri as usize];
                    output.push(match flip {
                        true => triangle.flip(),
                        false => *triangle,
                    });
                }
            }
            output
        }
    }
}
