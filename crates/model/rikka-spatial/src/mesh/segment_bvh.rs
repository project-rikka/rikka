use glam::{Vec2, Vec3, Vec3A};

use crate::aabb::Aabb;
use crate::bvh::{Bvh, BvhBuilder, BvhUpdate, Candidate};
use crate::closest::{closest_point_on_segment, closest_points_ray_segment};
use crate::error::{Error, Result};
use crate::ray::{Ray, RaySlab};
use crate::view::View;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentNearest {
    pub segment: u32,
    pub point: Vec3,
    pub parameter: f32,
    pub distance: f32,
}

impl Candidate for SegmentNearest {
    fn distance(&self) -> f32 {
        self.distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentRayNearest {
    pub segment: u32,
    pub point: Vec3,
    pub parameter: f32,
    pub ray_distance: f32,
    pub distance: f32,
}

impl Candidate for SegmentRayNearest {
    fn distance(&self) -> f32 {
        self.distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentScreenNearest {
    pub segment: u32,
    pub parameter: f32,
    pub screen_point: Vec2,
    pub distance: f32,
}

impl Candidate for SegmentScreenNearest {
    fn distance(&self) -> f32 {
        self.distance
    }
}

#[derive(Debug, Clone)]
pub struct SegmentBvh {
    bvh: Bvh,
    positions: Vec<Vec3>,
    segments: Vec<[u32; 2]>,
}

impl SegmentBvh {
    pub fn new(positions: Vec<Vec3>, segments: Vec<[u32; 2]>) -> Result<Self> {
        Self::with_builder(positions, segments, Bvh::builder())
    }

    pub fn with_builder(
        positions: Vec<Vec3>,
        segments: Vec<[u32; 2]>,
        builder: BvhBuilder,
    ) -> Result<Self> {
        if u32::try_from(segments.len()).is_err() {
            return Err(Error::TooManyElements {
                count: segments.len(),
            });
        }
        for (element, segment) in segments.iter().enumerate() {
            if let Some(&index) = segment
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
        let bounds = segment_bounds(&positions, &segments);
        Ok(Self {
            bvh: builder.build(&bounds),
            positions,
            segments,
        })
    }

    pub fn bvh(&self) -> &Bvh {
        &self.bvh
    }

    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    pub fn segments(&self) -> &[[u32; 2]] {
        &self.segments
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn segment(&self, segment: u32) -> [Vec3; 2] {
        self.segments[segment as usize].map(|index| self.positions[index as usize])
    }

    pub fn set_positions(&mut self, positions: &[Vec3]) -> BvhUpdate {
        assert_eq!(
            positions.len(),
            self.positions.len(),
            "vertex count must not change"
        );
        self.positions.copy_from_slice(positions);
        let bounds = segment_bounds(&self.positions, &self.segments);
        self.bvh.update(&bounds)
    }

    pub fn patch(&mut self, positions: &[(u32, Vec3)], refit: &[u32]) -> Result<()> {
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
        if let Some((element, &index)) = refit
            .iter()
            .enumerate()
            .find(|(_, index)| **index as usize >= self.segments.len())
        {
            return Err(Error::IndexOutOfRange {
                element,
                index,
                len: self.segments.len(),
            });
        }
        for &(index, position) in positions {
            self.positions[index as usize] = position;
        }
        let positions = &self.positions;
        let segments = &self.segments;
        self.bvh.refit_primitives(refit.iter().map(|&segment| {
            let [start, end] = segments[segment as usize];
            (
                segment,
                Aabb::from_segment(positions[start as usize], positions[end as usize]),
            )
        }));
        Ok(())
    }

    pub fn nearest_to_point(&self, point: Vec3, max_distance: f32) -> Option<SegmentNearest> {
        self.bvh
            .nearest_to_point(point, max_distance, |segment, limit| {
                let [start, end] = self.segment(segment);
                let closest = closest_point_on_segment(point, start, end);
                let distance = closest.point.distance(point);
                (distance <= limit).then_some(SegmentNearest {
                    segment,
                    point: closest.point,
                    parameter: closest.parameter,
                    distance,
                })
            })
    }

    pub fn nearest_to_ray(
        &self,
        ray: &Ray,
        max_ray_distance: f32,
        max_distance: f32,
    ) -> Option<SegmentRayNearest> {
        let slab = RaySlab::new(ray);
        self.bvh.nearest(
            max_distance,
            |bounds, limit| ray_box_distance(ray, &slab, max_ray_distance, bounds, limit),
            |segment, limit| {
                let pair = closest_points_ray_segment(ray, max_ray_distance, self.segment(segment));
                let distance = pair.distance_squared.sqrt();
                (distance <= limit).then_some(SegmentRayNearest {
                    segment,
                    point: pair.segment.point,
                    parameter: pair.segment.parameter,
                    ray_distance: pair.ray_distance,
                    distance,
                })
            },
        )
    }

    pub fn nearest_projected(
        &self,
        view: &View,
        screen: Vec2,
        max_distance: f32,
    ) -> Option<SegmentScreenNearest> {
        self.bvh.nearest(
            max_distance,
            |bounds, _| view.screen_distance_bound(bounds, screen),
            |segment, limit| {
                let [start, end] = self.segment(segment);
                let clipped = view.clip_segment(start, end)?;
                let flat = clipped.screen;
                let direction = flat.end - flat.start;
                let length_squared = direction.length_squared();
                let screen_parameter = match length_squared > 0.0 {
                    true => (direction.dot(screen - flat.start) / length_squared).clamp(0.0, 1.0),
                    false => 0.0,
                };
                let screen_point = flat.start + direction * screen_parameter;
                let distance = screen_point.distance(screen);
                (distance <= limit).then(|| SegmentScreenNearest {
                    segment,
                    parameter: clipped.segment_parameter(screen_parameter),
                    screen_point,
                    distance,
                })
            },
        )
    }
}

fn segment_bounds(positions: &[Vec3], segments: &[[u32; 2]]) -> Vec<Aabb> {
    segments
        .iter()
        .map(|&[start, end]| Aabb::from_segment(positions[start as usize], positions[end as usize]))
        .collect()
}

fn ray_box_distance(
    ray: &Ray,
    slab: &RaySlab,
    max_ray_distance: f32,
    bounds: &Aabb,
    limit: f32,
) -> f32 {
    let margin = Vec3A::splat(limit.min(f32::MAX));
    let (min, max) = (Vec3A::from(bounds.min), Vec3A::from(bounds.max));
    if slab
        .interval(min - margin, max + margin, 0.0, max_ray_distance)
        .is_none()
    {
        return f32::INFINITY;
    }
    if slab.interval(min, max, 0.0, max_ray_distance).is_some() {
        return 0.0;
    }
    let mut nearest = bounds.distance_squared_to_point(ray.origin());
    if max_ray_distance.is_finite() {
        nearest = nearest.min(bounds.distance_squared_to_point(ray.at(max_ray_distance)));
    }
    let corners = bounds.corners();
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (start, end) in EDGES {
        let pair =
            closest_points_ray_segment(ray, max_ray_distance, [corners[start], corners[end]]);
        nearest = nearest.min(pair.distance_squared);
    }
    nearest.sqrt()
}
