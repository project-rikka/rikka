use rikka_physics_geometry::{ProxyBuffer, Shape, ShapeProxy, project_point};
use rikka_physics_math::{Aabb, Transform, Vec3};

use super::QueryWorld;
use crate::{Material, QueryFilter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointHit {
    pub collider: u32,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub triangle: u32,
    pub child: u32,
    pub material: Material,
}

#[derive(Clone, Copy, Debug)]
struct Local {
    point: Vec3,
    normal: Vec3,
    distance: f32,
    triangle: u32,
    child: u32,
    material: u32,
}

impl Local {
    fn transformed(mut self, transform: Transform) -> Self {
        self.point = transform.apply(self.point);
        self.normal = transform.rotate(self.normal);
        self
    }
}

fn triangle(
    vertices: [Vec3; 3],
    point: Vec3,
    index: u32,
    material: u32,
    child: u32,
) -> Option<Local> {
    let projection = project_point(&ShapeProxy::new(&vertices, 0.0), point)?;
    Some(Local {
        point: projection.point,
        normal: projection.normal,
        distance: projection.distance,
        triangle: index,
        child,
        material,
    })
}

fn closer(best: &mut Option<Local>, limit: &mut f32, candidate: Option<Local>) {
    if let Some(candidate) = candidate
        && candidate.distance < *limit
    {
        *limit = candidate.distance;
        *best = Some(candidate);
    }
}

fn closest_local(shape: &Shape, point: Vec3, max_distance: f32, child: u32) -> Option<Local> {
    let mut best = None;
    let mut limit = max_distance;
    let region = |limit: f32| Aabb::from_center_extent(point, Vec3::splat(limit.max(0.0)));
    match shape {
        Shape::Mesh(mesh) => {
            mesh.query_triangles(region(limit), |a, b, c, index| {
                let material = mesh.triangle(index).material as u32;
                closer(
                    &mut best,
                    &mut limit,
                    triangle([a, b, c], point, index, material, child),
                );
                true
            });
        }
        Shape::HeightField(field) => {
            field.query_triangles(region(limit), |a, b, c, index| {
                let material = field.triangle(index).material as u32;
                closer(
                    &mut best,
                    &mut limit,
                    triangle([a, b, c], point, index, material, child),
                );
                true
            });
        }
        Shape::Compound(compound) => {
            compound.query_children(region(limit), |index| {
                let entry = compound.child(index);
                let local = entry.transform.apply_inverse(point);
                let found = closest_local(entry.shape, local, limit, index).map(|mut hit| {
                    hit.material = match entry.shape {
                        Shape::Mesh(_) => entry.material + hit.material,
                        _ => entry.material,
                    };
                    hit.transformed(entry.transform)
                });
                closer(&mut best, &mut limit, found);
                true
            });
        }
        Shape::Plane(plane) => {
            let distance = plane.separation(point);
            closer(
                &mut best,
                &mut limit,
                Some(Local {
                    point: plane.project(point),
                    normal: plane.normal,
                    distance,
                    triangle: 0,
                    child,
                    material: 0,
                }),
            );
        }
        _ => {
            let mut buffer = ProxyBuffer::default();
            let proxy = shape.proxy(&mut buffer)?;
            let found = project_point(&proxy, point).map(|projection| Local {
                point: projection.point,
                normal: projection.normal,
                distance: projection.distance,
                triangle: 0,
                child,
                material: 0,
            });
            closer(&mut best, &mut limit, found);
        }
    }
    best
}

impl QueryWorld<'_> {
    pub fn closest_point(
        &self,
        point: Vec3,
        max_distance: f32,
        filter: &QueryFilter,
    ) -> Option<PointHit> {
        let mut best: Option<PointHit> = None;
        self.broadphase
            .closest(point, max_distance, |index, limit| {
                let collider = &self.colliders[index as usize];
                if !self.accepts(index, collider, filter) {
                    return limit;
                }
                let frame = self.frame(collider);
                let Some(local) =
                    closest_local(&collider.shape, frame.apply_inverse(point), limit, 0)
                else {
                    return limit;
                };
                let world = local.transformed(frame);
                best = Some(PointHit {
                    collider: index,
                    point: world.point,
                    normal: world.normal,
                    distance: world.distance,
                    triangle: world.triangle,
                    child: world.child,
                    material: *collider.material(world.material),
                });
                world.distance.max(0.0)
            });
        best
    }
}
