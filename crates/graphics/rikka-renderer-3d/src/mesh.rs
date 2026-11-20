use glam::{Vec2, Vec3, Vec4};

use crate::arena::Span;
use crate::error::{Error, Result};
use crate::reach::{self, Geometry, Morphs, Part, Sphere};
use crate::vertex::{GpuAttribute, GpuMorphDelta, GpuPosition, GpuSkin, encode_octahedral};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshId(pub(crate) u64);

#[derive(Clone, Copy, Debug)]
pub struct Submesh {
    pub first_index: u32,
    pub index_count: u32,
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum IndexWidth {
    U16,
    #[default]
    U32,
}

impl IndexWidth {
    pub(crate) fn index_type(self) -> ash::vk::IndexType {
        match self {
            Self::U16 => ash::vk::IndexType::UINT16,
            Self::U32 => ash::vk::IndexType::UINT32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Indices<'a> {
    U16(&'a [u16]),
    U32(&'a [u32]),
}

impl Indices<'_> {
    pub fn len(&self) -> usize {
        match self {
            Self::U16(indices) => indices.len(),
            Self::U32(indices) => indices.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn width(&self) -> IndexWidth {
        match self {
            Self::U16(_) => IndexWidth::U16,
            Self::U32(_) => IndexWidth::U32,
        }
    }

    pub(crate) fn widened(&self) -> Vec<u32> {
        match self {
            Self::U16(indices) => indices.iter().map(|&i| u32::from(i)).collect(),
            Self::U32(indices) => indices.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MorphTarget<'a> {
    pub vertices: &'a [u32],
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MeshData<'a> {
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub tangents: &'a [Vec4],
    pub uvs: &'a [Vec2],
    pub colors: &'a [[u8; 4]],
    pub joints: &'a [[u16; 4]],
    pub weights: &'a [Vec4],
    pub outline_normals: &'a [Vec3],
    pub indices: Option<Indices<'a>>,
    pub submeshes: &'a [Submesh],
    pub morph_targets: &'a [MorphTarget<'a>],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MeshUpdate<'a> {
    pub first: u32,
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub tangents: &'a [Vec4],
    pub uvs: &'a [Vec2],
    pub colors: &'a [[u8; 4]],
}

impl MeshUpdate<'_> {
    pub(crate) fn count(&self, vertices: u32) -> Result<u32> {
        let lengths = [
            self.positions.len(),
            self.normals.len(),
            self.tangents.len(),
            self.uvs.len(),
            self.colors.len(),
        ];
        let count = lengths.into_iter().max().unwrap_or(0);
        if lengths.iter().any(|&length| length != 0 && length != count) {
            return Err(Error::Invalid(
                "mesh update attributes differ in length".into(),
            ));
        }
        let end = u64::from(self.first) + count as u64;
        if end > u64::from(vertices) {
            return Err(Error::Invalid(format!(
                "mesh update ends at vertex {end} of {vertices}"
            )));
        }
        Ok(count as u32)
    }
}

pub(crate) struct MeshEntry {
    pub vertices: Span,
    pub skins: Span,
    pub indices: Span,
    pub index_width: IndexWidth,
    pub parts: Vec<Part>,
    pub morph: MorphEntry,
    pub joints: u32,
    pub outline_normals: Span,
    pub edges: Vec<Option<Span>>,
    pub shape: Shape,
}

pub(crate) struct Shape {
    pub positions: Vec<Vec3>,
    pub skins: Vec<GpuSkin>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MorphEntry {
    pub starts: Span,
    pub deltas: Span,
    pub targets: u32,
}

impl MeshEntry {
    pub(crate) fn vertex_count(&self) -> u32 {
        self.vertices.count
    }

    pub(crate) fn has_skin(&self) -> bool {
        self.skins.count > 0
    }

    pub(crate) fn deforms(&self) -> bool {
        self.has_skin() || self.morph.targets > 0
    }
}

pub(crate) struct MeshBuild {
    pub positions: Vec<GpuPosition>,
    pub attributes: Vec<GpuAttribute>,
    pub skins: Vec<GpuSkin>,
    pub joints: u32,
    pub indices: Vec<u32>,
    pub index_width: IndexWidth,
    pub parts: Vec<Part>,
    pub morph: MorphBuild,
    pub outline_normals: Vec<u32>,
    pub shape: Vec<Vec3>,
}

pub(crate) struct Topology {
    pub indices: Vec<u32>,
    pub index_width: IndexWidth,
    pub submeshes: Vec<Submesh>,
}

pub(crate) fn topology(
    indices: Option<Indices<'_>>,
    submeshes: &[Submesh],
    positions: &[Vec3],
) -> Result<Topology> {
    let count = positions.len();
    let widened = match indices {
        Some(indices) => indices.widened(),
        None => (0..count as u32).collect(),
    };
    if u32::try_from(widened.len()).is_err() {
        return Err(Error::Invalid(format!(
            "mesh of {} indices exceeds the limit of {}",
            widened.len(),
            u32::MAX
        )));
    }
    if let Some(&worst) = widened.iter().max()
        && worst as usize >= count
    {
        return Err(Error::Invalid(format!(
            "index {worst} is out of range for {count} vertices"
        )));
    }
    let submeshes = match submeshes.is_empty() {
        true => {
            let Sphere { center, radius } = Sphere::around(positions);
            vec![Submesh {
                first_index: 0,
                index_count: widened.len() as u32,
                center,
                radius,
            }]
        }
        false => submeshes.to_vec(),
    };
    for submesh in &submeshes {
        let end = submesh.first_index as usize + submesh.index_count as usize;
        if end > widened.len() {
            return Err(Error::Invalid(format!(
                "submesh ends at index {end} of {}",
                widened.len()
            )));
        }
    }
    Ok(Topology {
        indices: widened,
        index_width: indices.map_or(IndexWidth::U32, |indices| indices.width()),
        submeshes,
    })
}

#[derive(Default)]
pub(crate) struct MorphBuild {
    pub starts: Vec<u32>,
    pub deltas: Vec<GpuMorphDelta>,
    pub targets: u32,
}

pub(crate) fn build(data: &MeshData<'_>) -> Result<MeshBuild> {
    let count = data.positions.len();
    if count == 0 {
        return Err(Error::Invalid("mesh has no vertices".into()));
    }
    if u32::try_from(count).is_err() {
        return Err(Error::Invalid(format!(
            "mesh of {count} vertices exceeds the limit of {}",
            u32::MAX
        )));
    }
    check_len("normals", data.normals.len(), count)?;
    check_len("tangents", data.tangents.len(), count)?;
    check_len("uvs", data.uvs.len(), count)?;
    check_len("colors", data.colors.len(), count)?;
    check_len("joints", data.joints.len(), count)?;
    check_len("weights", data.weights.len(), count)?;
    check_len("outline normals", data.outline_normals.len(), count)?;

    let Topology {
        indices,
        index_width,
        submeshes,
    } = topology(data.indices, data.submeshes, data.positions)?;
    let morph = morphs(data.morph_targets, count)?;

    let normals = match data.normals.is_empty() {
        true => derive_normals(data.positions, &indices),
        false => data.normals.to_vec(),
    };
    let tangents = match (data.tangents.is_empty(), data.uvs.is_empty()) {
        (false, _) => data.tangents.to_vec(),
        (true, false) => derive_tangents(data.positions, &normals, data.uvs, &indices),
        (true, true) => normals.iter().map(|&n| arbitrary_tangent(n)).collect(),
    };

    let positions = data
        .positions
        .iter()
        .map(|p| GpuPosition {
            position: p.to_array(),
        })
        .collect();
    let attributes = (0..count)
        .map(|i| {
            GpuAttribute::new(
                normals[i],
                tangents[i],
                data.uvs.get(i).copied().unwrap_or(Vec2::ZERO),
                data.colors.get(i).copied().unwrap_or([255; 4]),
            )
        })
        .collect();
    let skins: Vec<GpuSkin> = match data.joints.is_empty() || data.weights.is_empty() {
        true => Vec::new(),
        false => (0..count)
            .map(|i| GpuSkin::new(data.joints[i], data.weights[i]))
            .collect(),
    };
    let joints = skins
        .iter()
        .flat_map(GpuSkin::bound_joints)
        .max()
        .map_or(0, |joint| u32::from(joint) + 1);
    let parts = reach::parts(
        submeshes,
        &Geometry {
            positions: data.positions,
            indices: &indices,
            skins: &skins,
            joints,
            morphs: Morphs::Deltas(&morph),
        },
    );

    let outline_normals = data
        .outline_normals
        .iter()
        .zip(normals.iter().zip(&tangents))
        .map(|(&smoothed, (&normal, &tangent))| {
            encode_octahedral(tangent_space(smoothed, normal, tangent))
        })
        .collect();

    Ok(MeshBuild {
        positions,
        attributes,
        skins,
        joints,
        indices,
        index_width,
        parts,
        morph,
        outline_normals,
        shape: data.positions.to_vec(),
    })
}

fn tangent_space(direction: Vec3, normal: Vec3, tangent: Vec4) -> Vec3 {
    let n = normal.normalize_or(Vec3::Z);
    let t = tangent.truncate() - n * n.dot(tangent.truncate());
    let Some(t) = t.try_normalize() else {
        return Vec3::Z;
    };
    let b = n.cross(t) * tangent.w.signum();
    let d = direction.normalize_or(n);
    Vec3::new(d.dot(t), d.dot(b), d.dot(n))
}

fn morphs(targets: &[MorphTarget<'_>], count: usize) -> Result<MorphBuild> {
    if targets.is_empty() {
        return Ok(MorphBuild::default());
    }
    let Ok(target_count) = u32::try_from(targets.len()) else {
        return Err(Error::Invalid(format!(
            "{} morph targets exceed the limit of {}",
            targets.len(),
            u32::MAX
        )));
    };

    let mut starts = vec![0u32; count + 1];
    let mut total = 0u64;
    for target in targets {
        if target.vertices.len() != target.positions.len() {
            return Err(Error::Invalid(
                "morph target vertices and positions differ in length".into(),
            ));
        }
        if !target.normals.is_empty() && target.normals.len() != target.vertices.len() {
            return Err(Error::Invalid(
                "morph target normals must be empty or match its vertices".into(),
            ));
        }
        if !target.vertices.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(Error::Invalid(
                "morph target vertices must be sorted and unique".into(),
            ));
        }
        if let Some(&last) = target.vertices.last()
            && last as usize >= count
        {
            return Err(Error::Invalid(format!(
                "morph target vertex {last} is out of range for {count} vertices"
            )));
        }
        total += target.vertices.len() as u64;
        for &vertex in target.vertices {
            starts[vertex as usize + 1] += 1;
        }
    }
    if u32::try_from(total).is_err() {
        return Err(Error::Invalid(format!(
            "{total} morph deltas exceed the limit of {}",
            u32::MAX
        )));
    }

    for vertex in 0..count {
        starts[vertex + 1] += starts[vertex];
    }
    let mut cursor = starts.clone();
    let mut deltas = vec![GpuMorphDelta::default(); total as usize];
    for (index, target) in targets.iter().enumerate() {
        for (i, &vertex) in target.vertices.iter().enumerate() {
            let at = &mut cursor[vertex as usize];
            deltas[*at as usize] = GpuMorphDelta {
                target: index as u32,
                position: target.positions[i].to_array(),
                normal: target
                    .normals
                    .get(i)
                    .copied()
                    .unwrap_or_default()
                    .to_array(),
                _pad: 0,
            };
            *at += 1;
        }
    }

    Ok(MorphBuild {
        starts,
        deltas,
        targets: target_count,
    })
}

fn check_len(what: &str, actual: usize, expected: usize) -> Result<()> {
    if actual != 0 && actual != expected {
        return Err(Error::Invalid(format!(
            "{what}: {actual} entries for {expected} vertices"
        )));
    }
    Ok(())
}

fn derive_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let (a, b, c) = (
            positions[triangle[0] as usize],
            positions[triangle[1] as usize],
            positions[triangle[2] as usize],
        );
        let face = (b - a).cross(c - a);
        for &index in triangle {
            normals[index as usize] += face;
        }
    }
    for normal in &mut normals {
        *normal = normal.normalize_or(Vec3::Y);
    }
    normals
}

fn derive_tangents(
    positions: &[Vec3],
    normals: &[Vec3],
    uvs: &[Vec2],
    indices: &[u32],
) -> Vec<Vec4> {
    let mut tangents = vec![Vec3::ZERO; positions.len()];
    let mut bitangents = vec![Vec3::ZERO; positions.len()];

    for triangle in indices.as_chunks::<3>().0 {
        let (i0, i1, i2) = (
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        );
        let edge1 = positions[i1] - positions[i0];
        let edge2 = positions[i2] - positions[i0];
        let delta1 = uvs[i1] - uvs[i0];
        let delta2 = uvs[i2] - uvs[i0];

        let determinant = delta1.x * delta2.y - delta2.x * delta1.y;
        if determinant.abs() < 1e-12 {
            continue;
        }
        let inverse = 1.0 / determinant;
        let tangent = (edge1 * delta2.y - edge2 * delta1.y) * inverse;
        let bitangent = (edge2 * delta1.x - edge1 * delta2.x) * inverse;
        for &index in triangle {
            tangents[index as usize] += tangent;
            bitangents[index as usize] += bitangent;
        }
    }

    (0..positions.len())
        .map(|i| {
            let normal = normals[i];
            let tangent = tangents[i];
            let orthogonal = (tangent - normal * normal.dot(tangent)).normalize_or(Vec3::ZERO);
            if orthogonal == Vec3::ZERO {
                return arbitrary_tangent(normal);
            }
            let sign = match normal.cross(orthogonal).dot(bitangents[i]) < 0.0 {
                true => -1.0,
                false => 1.0,
            };
            orthogonal.extend(sign)
        })
        .collect()
}

fn arbitrary_tangent(normal: Vec3) -> Vec4 {
    let axis = match normal.x.abs() < 0.9 {
        true => Vec3::X,
        false => Vec3::Y,
    };
    normal.cross(axis).normalize_or(Vec3::X).extend(1.0)
}
