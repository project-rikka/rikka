use std::ops::Range;

use glam::{Mat4, Vec3};

use crate::mesh::{MorphBuild, Submesh};
use crate::vertex::GpuSkin;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub(crate) fn around(points: &[Vec3]) -> Self {
        let extent = points
            .iter()
            .fold(Extent::EMPTY, |extent, &point| extent.grown(point));
        let center = extent.center();
        let radius = points
            .iter()
            .map(|point| point.distance_squared(center))
            .fold(0.0, f32::max)
            .sqrt();
        Self { center, radius }
    }

    fn merged(self, other: Self) -> Self {
        let apart = self.center.distance(other.center);
        if apart + other.radius <= self.radius {
            return self;
        }
        if apart + self.radius <= other.radius {
            return other;
        }
        let radius = (apart + self.radius + other.radius) * 0.5;
        let center = self.center + (other.center - self.center) * ((radius - self.radius) / apart);
        Self { center, radius }
    }
}

#[derive(Clone, Copy)]
struct Extent {
    low: Vec3,
    high: Vec3,
}

impl Extent {
    const EMPTY: Self = Self {
        low: Vec3::splat(f32::INFINITY),
        high: Vec3::splat(f32::NEG_INFINITY),
    };

    fn grown(self, point: Vec3) -> Self {
        Self {
            low: self.low.min(point),
            high: self.high.max(point),
        }
    }

    fn is_empty(self) -> bool {
        self.low.x > self.high.x
    }

    fn center(self) -> Vec3 {
        (self.low + self.high) * 0.5
    }
}

#[derive(Clone)]
pub(crate) struct Part {
    pub submesh: Submesh,
    pub reach: Reach,
    pub vertices: Range<u32>,
}

impl Part {
    pub(crate) fn refit(&mut self, positions: &[Vec3], skins: &[GpuSkin], joints: u32) {
        let drawn = self.vertices.start as usize..self.vertices.end as usize;
        if drawn.is_empty() {
            return;
        }
        let Sphere { center, radius } = Sphere::around(&positions[drawn.clone()]);
        self.submesh.center = center;
        self.submesh.radius = radius;
        if !skins.is_empty() {
            self.reach.joints =
                joint_spheres(positions, skins, joints, drawn.map(|vertex| vertex as u32));
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Reach {
    joints: Vec<JointSphere>,
    morphs: Vec<MorphReach>,
}

#[derive(Clone, Copy)]
struct JointSphere {
    joint: u32,
    sphere: Sphere,
}

#[derive(Clone, Copy)]
pub(crate) struct MorphReach {
    target: u32,
    distance: f32,
}

#[derive(Clone, Copy)]
pub(crate) enum Morphs<'a> {
    Deltas(&'a MorphBuild),
    Bounded(&'a [MorphReach]),
}

impl Morphs<'_> {
    fn is_empty(self) -> bool {
        match self {
            Self::Deltas(morph) => morph.targets == 0,
            Self::Bounded(reaches) => reaches.is_empty(),
        }
    }
}

pub(crate) struct Geometry<'a> {
    pub positions: &'a [Vec3],
    pub indices: &'a [u32],
    pub skins: &'a [GpuSkin],
    pub joints: u32,
    pub morphs: Morphs<'a>,
}

pub(crate) struct Pose<'a> {
    pub bones: Option<&'a [Mat4]>,
    pub morph_weights: &'a [f32],
}

pub(crate) fn parts(submeshes: Vec<Submesh>, geometry: &Geometry<'_>) -> Vec<Part> {
    let mut measure = Measure::new(geometry);
    submeshes
        .into_iter()
        .map(|submesh| {
            measure.gather(&submesh);
            let reach = match geometry.skins.is_empty() && geometry.morphs.is_empty() {
                true => Reach::default(),
                false => Reach {
                    joints: measure.joint_spheres(),
                    morphs: measure.morph_reaches(),
                },
            };
            Part {
                vertices: measure.span(),
                reach,
                submesh,
            }
        })
        .collect()
}

pub(crate) fn morph_bound(parts: &[Part]) -> Vec<MorphReach> {
    let mut bound: Vec<MorphReach> = Vec::new();
    for morph in parts.iter().flat_map(|part| &part.reach.morphs) {
        match bound.iter_mut().find(|known| known.target == morph.target) {
            Some(known) => known.distance = known.distance.max(morph.distance),
            None => bound.push(*morph),
        }
    }
    bound
}

fn joint_spheres(
    positions: &[Vec3],
    skins: &[GpuSkin],
    joints: u32,
    vertices: impl Iterator<Item = u32> + Clone,
) -> Vec<JointSphere> {
    let mut extents = vec![Extent::EMPTY; joints as usize];
    let mut radii = vec![0.0f32; joints as usize];
    for vertex in vertices.clone() {
        for joint in skins[vertex as usize].bound_joints() {
            let extent = &mut extents[usize::from(joint)];
            *extent = extent.grown(positions[vertex as usize]);
        }
    }
    for vertex in vertices {
        for joint in skins[vertex as usize].bound_joints() {
            let joint = usize::from(joint);
            let distance = positions[vertex as usize].distance(extents[joint].center());
            radii[joint] = radii[joint].max(distance);
        }
    }
    extents
        .iter()
        .zip(&radii)
        .enumerate()
        .filter(|(_, (extent, _))| !extent.is_empty())
        .map(|(joint, (extent, &radius))| JointSphere {
            joint: joint as u32,
            sphere: Sphere {
                center: extent.center(),
                radius,
            },
        })
        .collect()
}

struct Measure<'a> {
    geometry: &'a Geometry<'a>,
    seen: Vec<bool>,
    vertices: Vec<u32>,
    distances: Vec<f32>,
}

impl<'a> Measure<'a> {
    fn new(geometry: &'a Geometry<'a>) -> Self {
        Self {
            geometry,
            seen: vec![false; geometry.positions.len()],
            vertices: Vec::new(),
            distances: Vec::new(),
        }
    }

    fn gather(&mut self, submesh: &Submesh) {
        let start = submesh.first_index as usize;
        let drawn = &self.geometry.indices[start..start + submesh.index_count as usize];
        self.vertices.clear();
        for &vertex in drawn {
            if !std::mem::replace(&mut self.seen[vertex as usize], true) {
                self.vertices.push(vertex);
            }
        }
        for &vertex in &self.vertices {
            self.seen[vertex as usize] = false;
        }
    }

    fn span(&self) -> Range<u32> {
        let low = self.vertices.iter().copied().min();
        let high = self.vertices.iter().copied().max();
        match low.zip(high) {
            Some((low, high)) => low..high + 1,
            None => 0..0,
        }
    }

    fn joint_spheres(&self) -> Vec<JointSphere> {
        let Geometry {
            positions,
            skins,
            joints,
            ..
        } = *self.geometry;
        if skins.is_empty() {
            return Vec::new();
        }
        joint_spheres(positions, skins, joints, self.vertices.iter().copied())
    }

    fn morph_reaches(&mut self) -> Vec<MorphReach> {
        let morph = match self.geometry.morphs {
            Morphs::Deltas(morph) if morph.targets > 0 => morph,
            Morphs::Deltas(_) => return Vec::new(),
            Morphs::Bounded(reaches) => return reaches.to_vec(),
        };
        self.distances.clear();
        self.distances.resize(morph.targets as usize, 0.0);
        for &vertex in &self.vertices {
            let first = morph.starts[vertex as usize] as usize;
            let last = morph.starts[vertex as usize + 1] as usize;
            for delta in &morph.deltas[first..last] {
                let distance = &mut self.distances[delta.target as usize];
                *distance = distance.max(Vec3::from_array(delta.position).length());
            }
        }
        self.distances
            .iter()
            .enumerate()
            .filter(|&(_, &distance)| distance > 0.0)
            .map(|(target, &distance)| MorphReach {
                target: target as u32,
                distance,
            })
            .collect()
    }
}

impl Reach {
    pub(crate) fn posed(&self, rest: Sphere, pose: &Pose<'_>) -> Sphere {
        let slack = self
            .morphs
            .iter()
            .filter_map(|morph| {
                pose.morph_weights
                    .get(morph.target as usize)
                    .map(|weight| weight.abs() * morph.distance)
            })
            .sum::<f32>();
        let skinned = pose.bones.and_then(|bones| {
            self.joints
                .iter()
                .map(|joint_sphere| {
                    let bone = bones[joint_sphere.joint as usize];
                    Sphere {
                        center: bone.transform_point3(joint_sphere.sphere.center),
                        radius: (joint_sphere.sphere.radius + slack) * largest_stretch(bone),
                    }
                })
                .reduce(Sphere::merged)
        });
        skinned.unwrap_or(Sphere {
            center: rest.center,
            radius: rest.radius + slack,
        })
    }
}

pub(crate) fn largest_stretch(transform: Mat4) -> f32 {
    let x = transform.x_axis.truncate();
    let y = transform.y_axis.truncate();
    let z = transform.z_axis.truncate();
    let (xy, xz, yz) = (x.dot(y).abs(), x.dot(z).abs(), y.dot(z).abs());
    (x.length_squared() + xy + xz)
        .max(y.length_squared() + xy + yz)
        .max(z.length_squared() + xz + yz)
        .sqrt()
}
