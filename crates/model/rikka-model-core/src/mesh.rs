use glam::{Vec2, Vec3, Vec4};
use rustc_hash::FxHashMap;

use crate::bounds::Bounds;
use crate::coordinates::Conversion;
use crate::error::{Error, Result};
use crate::id::model_id;
use crate::material::MaterialId;
use crate::polygons::Polygons;

model_id!(MeshId);

pub const INFLUENCES: usize = 4;

#[derive(Clone, Debug, Default)]
pub struct Vertices {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec4>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<[u8; 4]>,
    pub joints: Vec<[u16; INFLUENCES]>,
    pub weights: Vec<Vec4>,
}

impl Vertices {
    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    #[must_use]
    pub fn is_skinned(&self) -> bool {
        !self.joints.is_empty() && !self.weights.is_empty()
    }

    #[must_use]
    pub fn bounds(&self) -> Bounds {
        Bounds::from_points(self.positions.iter().copied())
    }

    pub fn convert(&mut self, conversion: &Conversion) {
        for position in &mut self.positions {
            *position = conversion.point(*position);
        }
        for normal in &mut self.normals {
            *normal = conversion.vector(*normal).normalize_or(Vec3::Y);
        }
        let handedness = match conversion.flips_winding() {
            true => -1.0,
            false => 1.0,
        };
        for tangent in &mut self.tangents {
            let direction = conversion.vector(tangent.truncate()).normalize_or(Vec3::X);
            *tangent = direction.extend(tangent.w * handedness);
        }
    }

    pub fn normalize_weights(&mut self) {
        for (joints, weights) in self.joints.iter_mut().zip(&mut self.weights) {
            let total = weights.max(Vec4::ZERO).element_sum();
            if total <= 1e-6 {
                *weights = Vec4::X;
                joints[1..].fill(0);
                continue;
            }
            for slot in 0..INFLUENCES {
                if weights[slot] <= 0.0 {
                    weights[slot] = 0.0;
                    joints[slot] = 0;
                }
            }
            *weights /= total;
        }
    }

    #[must_use]
    pub fn influences(&self, vertex: usize) -> Option<[(u16, f32); INFLUENCES]> {
        let joints = self.joints.get(vertex)?;
        let weights = self.weights.get(vertex)?.to_array();
        Some(std::array::from_fn(|slot| (joints[slot], weights[slot])))
    }

    pub fn set_influences(&mut self, vertex: usize, influences: &[(u16, f32)]) {
        let count = self.len().max(vertex + 1);
        self.joints.resize(count, [0; INFLUENCES]);
        self.weights.resize(count, Vec4::ZERO);
        let strongest = strongest(influences);
        let mut joints = [0; INFLUENCES];
        let mut weights = Vec4::ZERO;
        for (slot, (joint, weight)) in strongest.into_iter().flatten().enumerate() {
            joints[slot] = joint;
            weights[slot] = weight;
        }
        self.joints[vertex] = joints;
        self.weights[vertex] = weights;
    }

    pub(crate) fn validate(&self, mesh: &str) -> Result<()> {
        let count = self.len();
        let check = |what: &str, actual: usize| match actual == 0 || actual == count {
            true => Ok(()),
            false => Err(Error::Invalid(format!(
                "mesh \"{mesh}\": {actual} {what} for {count} vertices"
            ))),
        };
        check("normals", self.normals.len())?;
        check("tangents", self.tangents.len())?;
        check("uvs", self.uvs.len())?;
        check("colors", self.colors.len())?;
        check("joints", self.joints.len())?;
        check("weights", self.weights.len())
    }
}

fn strongest(influences: &[(u16, f32)]) -> [Option<(u16, f32)>; INFLUENCES] {
    let mut strongest = [None; INFLUENCES];
    if influences.len() <= INFLUENCES {
        for (slot, influence) in strongest.iter_mut().zip(influences) {
            *slot = Some(*influence);
        }
        return strongest;
    }
    for &(joint, weight) in influences {
        let Some(at) = strongest.iter().position(|slot| {
            slot.is_none_or(|(_, slot_weight)| weight.total_cmp(&slot_weight).is_gt())
        }) else {
            continue;
        };
        strongest.copy_within(at..INFLUENCES - 1, at + 1);
        strongest[at] = Some((joint, weight));
    }
    strongest
}

#[derive(Clone, Debug, Default)]
pub struct Submesh {
    pub name: String,
    pub first_index: u32,
    pub index_count: u32,
    pub material: Option<MaterialId>,
    pub bounds: Bounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeats {
    KeepFirst,
    Sum,
}

#[derive(Clone, Debug, Default)]
pub struct MorphTarget {
    pub name: String,
    pub vertices: Vec<u32>,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

impl MorphTarget {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn push(&mut self, vertex: u32, position: Vec3, normal: Option<Vec3>) {
        self.vertices.push(vertex);
        self.positions.push(position);
        self.normals.extend(normal);
    }

    pub fn convert(&mut self, conversion: &Conversion) {
        for position in &mut self.positions {
            *position = conversion.point(*position);
        }
        for normal in &mut self.normals {
            *normal = conversion.vector(*normal);
        }
    }

    pub fn merge(&mut self, repeats: Repeats) {
        if self.normals.len() != self.vertices.len() {
            self.normals.clear();
        }
        if self.vertices.windows(2).all(|pair| pair[0] < pair[1]) {
            return;
        }
        let mut order: Vec<usize> = (0..self.vertices.len()).collect();
        order.sort_by_key(|&index| self.vertices[index]);

        let shaped = !self.normals.is_empty();
        let mut vertices = Vec::with_capacity(order.len());
        let mut positions: Vec<Vec3> = Vec::with_capacity(order.len());
        let mut normals: Vec<Vec3> = Vec::with_capacity(if shaped { order.len() } else { 0 });
        for index in order {
            let vertex = self.vertices[index];
            let normal = self.normals.get(index).copied();
            if vertices.last() != Some(&vertex) {
                vertices.push(vertex);
                positions.push(self.positions[index]);
                normals.extend(normal);
                continue;
            }
            if repeats == Repeats::Sum {
                *positions.last_mut().expect("every vertex has a position") +=
                    self.positions[index];
                if let (Some(sum), Some(normal)) = (normals.last_mut(), normal) {
                    *sum += normal;
                }
            }
        }
        self.vertices = vertices;
        self.positions = positions;
        self.normals = normals;
    }

    pub(crate) fn validate(&self, mesh: &str, vertex_count: usize) -> Result<()> {
        let count = self.vertices.len();
        if self.positions.len() != count
            || (!self.normals.is_empty() && self.normals.len() != count)
        {
            return Err(Error::Invalid(format!(
                "morph target \"{}\" of mesh \"{mesh}\": {count} vertices with {} position and {} normal deltas",
                self.name,
                self.positions.len(),
                self.normals.len()
            )));
        }
        if let Some(&worst) = self.vertices.iter().max()
            && worst as usize >= vertex_count
        {
            return Err(Error::Invalid(format!(
                "morph target \"{}\" of mesh \"{mesh}\": vertex {worst} out of range for {vertex_count}",
                self.name
            )));
        }
        if !self.vertices.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(Error::Invalid(format!(
                "morph target \"{}\" of mesh \"{mesh}\": vertices are unsorted or repeated",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vertices,
    pub indices: Vec<u32>,
    pub submeshes: Vec<Submesh>,
    pub morph_targets: Vec<MorphTarget>,
    pub polygons: Option<Polygons>,
}

impl Mesh {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    #[must_use]
    pub fn bounds(&self) -> Bounds {
        self.vertices.bounds()
    }

    pub fn finalize(&mut self) {
        if self.submeshes.is_empty() {
            self.submeshes.push(Submesh {
                index_count: self.indices.len() as u32,
                ..Submesh::default()
            });
        }
        if self
            .submeshes
            .iter()
            .any(|submesh| submesh.bounds.is_empty())
        {
            self.recompute_bounds();
        }
    }

    pub fn recompute_bounds(&mut self) {
        for submesh in &mut self.submeshes {
            let first = submesh.first_index as usize;
            let end = first + submesh.index_count as usize;
            submesh.bounds = Bounds::from_points(
                self.indices[first.min(self.indices.len())..end.min(self.indices.len())]
                    .iter()
                    .filter_map(|&index| self.vertices.positions.get(index as usize).copied()),
            );
        }
    }

    pub fn flip_winding(&mut self) {
        for triangle in self.indices.as_chunks_mut::<3>().0 {
            triangle.swap(1, 2);
        }
        if let Some(polygons) = &mut self.polygons {
            polygons.flip_winding();
        }
    }

    #[must_use]
    pub fn smoothed_normals(&self) -> Vec<Vec3> {
        let positions = &self.vertices.positions;
        let key = |position: Vec3| position.to_array().map(f32::to_bits);
        let mut welded: FxHashMap<[u32; 3], u32> = FxHashMap::default();
        let corners: Vec<u32> = positions
            .iter()
            .map(|&position| {
                let next = welded.len() as u32;
                *welded.entry(key(position)).or_insert(next)
            })
            .collect();
        let mut sums = vec![Vec3::ZERO; welded.len()];
        for triangle in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = triangle.map(|index| index as usize);
            let (Some(&pa), Some(&pb), Some(&pc)) =
                (positions.get(a), positions.get(b), positions.get(c))
            else {
                continue;
            };
            let Some(face) = (pb - pa).cross(pc - pa).try_normalize() else {
                continue;
            };
            for (corner, from, to, other) in [(a, pa, pb, pc), (b, pb, pc, pa), (c, pc, pa, pb)] {
                let angle = (to - from).angle_between(other - from);
                if angle.is_finite() {
                    sums[corners[corner] as usize] += face * angle;
                }
            }
        }
        corners
            .iter()
            .enumerate()
            .map(|(vertex, &corner)| {
                let fallback = self
                    .vertices
                    .normals
                    .get(vertex)
                    .copied()
                    .unwrap_or(Vec3::Y);
                sums[corner as usize].try_normalize().unwrap_or(fallback)
            })
            .collect()
    }

    pub(crate) fn validate(&self, materials: usize) -> Result<()> {
        self.vertices.validate(&self.name)?;
        if self.vertices.is_empty() {
            return Err(Error::Invalid(format!(
                "mesh \"{}\" has no vertices",
                self.name
            )));
        }
        if !self.indices.len().is_multiple_of(3) {
            return Err(Error::Invalid(format!(
                "mesh \"{}\": {} indices is not a multiple of 3",
                self.name,
                self.indices.len()
            )));
        }
        let count = self.vertices.len();
        if let Some(&worst) = self.indices.iter().max()
            && worst as usize >= count
        {
            return Err(Error::Invalid(format!(
                "mesh \"{}\" indexes vertex {worst} of {count}",
                self.name
            )));
        }
        for submesh in &self.submeshes {
            let end = submesh.first_index as usize + submesh.index_count as usize;
            if end > self.indices.len() {
                return Err(Error::Invalid(format!(
                    "submesh \"{}\" of mesh \"{}\" reaches index {end} of {}",
                    submesh.name,
                    self.name,
                    self.indices.len()
                )));
            }
            if let Some(material) = submesh.material
                && material.index() >= materials
            {
                return Err(Error::Invalid(format!(
                    "submesh \"{}\" of mesh \"{}\" references missing {material}",
                    submesh.name, self.name
                )));
            }
        }
        for target in &self.morph_targets {
            target.validate(&self.name, count)?;
        }
        if let Some(polygons) = &self.polygons {
            polygons.validate(&self.name, count, self.submeshes.len())?;
        }
        Ok(())
    }
}
