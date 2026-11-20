use glam::{Affine3A, Vec3};

use crate::aabb::Aabb;
use crate::bvh::{Bvh, BvhBuilder, BvhUpdate, Candidate};
use crate::closest::closest_point_on_triangle;
use crate::error::{Error, Result};
use crate::plane::{Containment, Frustum};
use crate::ray::{Culling, Ray, WatertightRay};
use crate::triangle::{TriangleIntersection, intersect_triangles};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangleHit {
    pub triangle: u32,
    pub distance: f32,
    pub point: Vec3,
    pub barycentric: Vec3,
    pub normal: Vec3,
}

impl Candidate for TriangleHit {
    fn distance(&self) -> f32 {
        self.distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangleNearest {
    pub triangle: u32,
    pub point: Vec3,
    pub barycentric: Vec3,
    pub distance: f32,
}

impl Candidate for TriangleNearest {
    fn distance(&self) -> f32 {
        self.distance
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrianglePair {
    pub first: u32,
    pub second: u32,
    pub intersection: TriangleIntersection,
}

#[derive(Debug, Clone)]
pub struct TriangleBvh {
    bvh: Bvh,
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
}

impl TriangleBvh {
    pub fn new(positions: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Result<Self> {
        Self::with_builder(positions, triangles, Bvh::builder())
    }

    pub fn with_builder(
        positions: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
        builder: BvhBuilder,
    ) -> Result<Self> {
        if u32::try_from(triangles.len()).is_err() {
            return Err(Error::TooManyElements {
                count: triangles.len(),
            });
        }
        for (element, triangle) in triangles.iter().enumerate() {
            if let Some(&index) = triangle
                .iter()
                .find(|&&index| index as usize >= positions.len())
            {
                return Err(Error::IndexOutOfRange {
                    element,
                    index,
                    len: positions.len(),
                });
            }
        }
        let bounds = triangle_bounds(&positions, &triangles);
        Ok(Self {
            bvh: builder.build(&bounds),
            positions,
            triangles,
        })
    }

    pub fn from_triangles(triangles: impl IntoIterator<Item = [Vec3; 3]>) -> Result<Self> {
        let positions: Vec<Vec3> = triangles.into_iter().flatten().collect();
        let count = positions.len() / 3;
        if u32::try_from(positions.len()).is_err() {
            return Err(Error::TooManyElements { count });
        }
        let indices = (0..count as u32)
            .map(|triangle| [triangle * 3, triangle * 3 + 1, triangle * 3 + 2])
            .collect();
        Self::new(positions, indices)
    }

    pub fn bvh(&self) -> &Bvh {
        &self.bvh
    }

    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    pub fn triangle(&self, triangle: u32) -> [Vec3; 3] {
        self.triangles[triangle as usize].map(|index| self.positions[index as usize])
    }

    pub fn set_positions(&mut self, positions: &[Vec3]) -> BvhUpdate {
        assert_eq!(
            positions.len(),
            self.positions.len(),
            "vertex count must not change"
        );
        self.positions.copy_from_slice(positions);
        let bounds = triangle_bounds(&self.positions, &self.triangles);
        self.bvh.update(&bounds)
    }

    pub fn patch(
        &mut self,
        positions: &[(u32, Vec3)],
        triangles: &[(u32, [u32; 3])],
        refit: &[u32],
    ) -> Result<()> {
        let len = self.positions.len();
        for (element, &(index, _)) in positions.iter().enumerate() {
            if index as usize >= len {
                return Err(Error::IndexOutOfRange {
                    element,
                    index,
                    len,
                });
            }
        }
        for (element, &(triangle, corners)) in triangles.iter().enumerate() {
            if let Some(&index) = corners.iter().find(|&&index| index as usize >= len) {
                return Err(Error::IndexOutOfRange {
                    element,
                    index,
                    len,
                });
            }
            if triangle as usize >= self.triangles.len() {
                return Err(Error::IndexOutOfRange {
                    element,
                    index: triangle,
                    len: self.triangles.len(),
                });
            }
        }
        if let Some((element, &index)) = refit
            .iter()
            .enumerate()
            .find(|(_, index)| **index as usize >= self.triangles.len())
        {
            return Err(Error::IndexOutOfRange {
                element,
                index,
                len: self.triangles.len(),
            });
        }
        for &(index, position) in positions {
            self.positions[index as usize] = position;
        }
        for &(triangle, corners) in triangles {
            self.triangles[triangle as usize] = corners;
        }
        let positions = &self.positions;
        let triangles = &self.triangles;
        self.bvh.refit_primitives(refit.iter().map(|&triangle| {
            let corners = triangles[triangle as usize].map(|index| positions[index as usize]);
            (triangle, Aabb::from_triangle(corners))
        }));
        Ok(())
    }

    pub fn ray_cast(&self, ray: &Ray, max_distance: f32, culling: Culling) -> Option<TriangleHit> {
        let watertight = WatertightRay::new(ray);
        self.bvh.ray_cast(ray, max_distance, |triangle, limit| {
            self.hit(ray, &watertight, triangle, limit, culling)
        })
    }

    pub fn ray_cast_any(&self, ray: &Ray, max_distance: f32, culling: Culling) -> bool {
        let watertight = WatertightRay::new(ray);
        self.bvh.ray_cast_any(ray, max_distance, |triangle, limit| {
            watertight
                .intersect_triangle(self.triangle(triangle), limit, culling)
                .is_some()
        })
    }

    pub fn ray_cast_all(&self, ray: &Ray, max_distance: f32, culling: Culling) -> Vec<TriangleHit> {
        let watertight = WatertightRay::new(ray);
        let mut hits = Vec::new();
        self.bvh.ray_cast_all(ray, max_distance, |triangle| {
            if let Some(hit) = self.hit(ray, &watertight, triangle, max_distance, culling) {
                hits.push(hit);
            }
        });
        hits.sort_by(|left, right| {
            left.distance
                .total_cmp(&right.distance)
                .then(left.triangle.cmp(&right.triangle))
        });
        hits
    }

    fn hit(
        &self,
        ray: &Ray,
        watertight: &WatertightRay,
        triangle: u32,
        limit: f32,
        culling: Culling,
    ) -> Option<TriangleHit> {
        let vertices = self.triangle(triangle);
        let hit = watertight.intersect_triangle(vertices, limit, culling)?;
        let [a, b, c] = vertices;
        Some(TriangleHit {
            triangle,
            distance: hit.distance,
            point: ray.at(hit.distance),
            barycentric: hit.barycentric,
            normal: (b - a).cross(c - a).normalize_or_zero(),
        })
    }

    pub fn closest_point(&self, point: Vec3, max_distance: f32) -> Option<TriangleNearest> {
        self.bvh
            .nearest_to_point(point, max_distance, |triangle, limit| {
                let closest = closest_point_on_triangle(point, self.triangle(triangle));
                let distance = closest.point.distance(point);
                (distance <= limit).then_some(TriangleNearest {
                    triangle,
                    point: closest.point,
                    barycentric: closest.barycentric,
                    distance,
                })
            })
    }

    pub fn for_each_in_sphere(&self, center: Vec3, radius: f32, mut visit: impl FnMut(u32)) {
        let radius_squared = radius * radius;
        self.bvh.for_each_in_sphere(center, radius, |triangle| {
            let closest = closest_point_on_triangle(center, self.triangle(triangle));
            if closest.point.distance_squared(center) <= radius_squared {
                visit(triangle);
            }
        });
    }

    pub fn for_each_in_frustum(&self, frustum: &Frustum, mut visit: impl FnMut(u32, Containment)) {
        self.bvh
            .for_each_in_frustum(frustum, |triangle, containment| {
                let containment = match containment {
                    Containment::Inside => Containment::Inside,
                    _ => frustum.classify_triangle(self.triangle(triangle)),
                };
                if containment != Containment::Outside {
                    visit(triangle, containment);
                }
            });
    }

    pub fn for_each_intersection(
        &self,
        other: &TriangleBvh,
        transform: &Affine3A,
        mut visit: impl FnMut(TrianglePair),
    ) {
        self.bvh
            .for_each_overlap(&other.bvh, transform, |first, second| {
                let other_triangle = other
                    .triangle(second)
                    .map(|vertex| transform.transform_point3(vertex));
                if let Some(intersection) =
                    intersect_triangles(self.triangle(first), other_triangle)
                {
                    visit(TrianglePair {
                        first,
                        second,
                        intersection,
                    });
                }
            });
    }

    pub fn for_each_self_intersection(&self, mut visit: impl FnMut(TrianglePair)) {
        self.bvh.for_each_self_overlap(|first, second| {
            let first_indices = self.triangles[first as usize];
            let second_indices = self.triangles[second as usize];
            if first_indices
                .iter()
                .any(|index| second_indices.contains(index))
            {
                return;
            }
            if let Some(intersection) =
                intersect_triangles(self.triangle(first), self.triangle(second))
            {
                visit(TrianglePair {
                    first,
                    second,
                    intersection,
                });
            }
        });
    }
}

fn triangle_bounds(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Aabb> {
    triangles
        .iter()
        .map(|triangle| Aabb::from_triangle(triangle.map(|index| positions[index as usize])))
        .collect()
}
