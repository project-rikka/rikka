use bytemuck::{Pod, Zeroable};
use rikka_physics_core::{Blob, BlobWriter, Section};
use rikka_physics_math::{Aabb, Vec3};

use crate::triangulated::bvh::{DecodedBounds, DecodedTree, QuantizedNode4, QuantizedTree};
use crate::triangulated::triangle::{EdgeFlags, Triangle, map3};
use crate::{Error, Result};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct MeshHeader {
    pub(crate) bounds: Aabb,
    pub(crate) surface_area: f32,
    pub(crate) material_count: u32,
    pub(crate) closed: u32,
    pub(crate) degenerate_count: u32,
    pub(crate) nodes: Section<QuantizedNode4>,
    pub(crate) vertices: Section<Vec3>,
    pub(crate) triangles: Section<[u32; 3]>,
    pub(crate) materials: Section<u8>,
    pub(crate) flags: Section<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    blob: Blob<MeshHeader>,
    decoded: DecodedBounds,
}

pub(crate) struct MeshParts<'a> {
    pub(crate) bounds: Aabb,
    pub(crate) surface_area: f32,
    pub(crate) material_count: u32,
    pub(crate) closed: bool,
    pub(crate) degenerate_count: u32,
    pub(crate) nodes: &'a [QuantizedNode4],
    pub(crate) vertices: &'a [Vec3],
    pub(crate) triangles: &'a [[u32; 3]],
    pub(crate) materials: &'a [u8],
    pub(crate) flags: &'a [u8],
}

impl TriangleMesh {
    pub(crate) fn assemble(parts: MeshParts<'_>) -> Self {
        let mut writer = BlobWriter::<MeshHeader>::new();
        let nodes = writer.push(parts.nodes);
        let vertices = writer.push(parts.vertices);
        let triangles = writer.push(parts.triangles);
        let materials = writer.push(parts.materials);
        let flags = writer.push(parts.flags);
        Self::with_blob(writer.finish(MeshHeader {
            bounds: parts.bounds,
            surface_area: parts.surface_area,
            material_count: parts.material_count,
            closed: u32::from(parts.closed),
            degenerate_count: parts.degenerate_count,
            nodes,
            vertices,
            triangles,
            materials,
            flags,
        }))
    }

    fn with_blob(blob: Blob<MeshHeader>) -> Self {
        let header = *blob.header();
        let decoded = DecodedBounds::decode(blob.section(header.nodes), header.bounds);
        Self { blob, decoded }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let blob = Blob::<MeshHeader>::from_bytes(bytes)?;
        let header = *blob.header();
        let nodes = blob.try_section(header.nodes)?;
        let vertices = blob.try_section(header.vertices)?;
        let triangles = blob.try_section(header.triangles)?;
        let materials = blob.try_section(header.materials)?;
        let flags = blob.try_section(header.flags)?;
        let count = triangles.len();
        let vertex_count = vertices.len();
        if count == 0 || nodes.is_empty() || materials.len() != count || flags.len() != count {
            return Err(Error::CorruptShape { shape: "mesh" });
        }
        if let Some(&index) = triangles
            .iter()
            .flatten()
            .find(|&&index| index as usize >= vertex_count)
        {
            return Err(Error::IndexOutOfRange {
                index,
                vertices: vertex_count,
            });
        }
        if !QuantizedTree::new(nodes, header.bounds).is_consistent(count as u32) {
            return Err(Error::CorruptShape { shape: "mesh" });
        }
        Ok(Self::with_blob(blob))
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        self.blob.bytes()
    }

    #[inline]
    pub fn content_hash(&self) -> u64 {
        self.blob.content_hash()
    }

    #[inline]
    fn header(&self) -> &MeshHeader {
        self.blob.header()
    }

    #[inline]
    pub fn bounds(&self) -> Aabb {
        self.header().bounds
    }

    #[inline]
    pub fn surface_area(&self) -> f32 {
        self.header().surface_area
    }

    #[inline]
    pub fn material_count(&self) -> u32 {
        self.header().material_count
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.header().closed != 0
    }

    #[inline]
    pub fn degenerate_count(&self) -> u32 {
        self.header().degenerate_count
    }

    #[inline]
    pub fn vertices(&self) -> &[Vec3] {
        self.blob.section(self.header().vertices)
    }

    #[inline]
    pub fn triangles(&self) -> &[[u32; 3]] {
        self.blob.section(self.header().triangles)
    }

    #[inline]
    pub fn materials(&self) -> &[u8] {
        self.blob.section(self.header().materials)
    }

    #[inline]
    pub fn edge_flags(&self) -> &[u8] {
        self.blob.section(self.header().flags)
    }

    #[inline]
    pub fn triangle_count(&self) -> u32 {
        self.header().triangles.len
    }

    #[inline]
    pub fn tree(&self) -> QuantizedTree<'_> {
        QuantizedTree::new(self.blob.section(self.header().nodes), self.bounds())
    }

    #[inline]
    pub fn triangle(&self, index: u32) -> Triangle {
        self.parts().0.triangle(index)
    }

    #[inline]
    pub(crate) fn parts(&self) -> (MeshSlices<'_>, DecodedTree<'_>) {
        let header = self.header();
        let slices = MeshSlices {
            vertices: self.blob.section(header.vertices),
            triangles: self.blob.section(header.triangles),
            materials: self.blob.section(header.materials),
            flags: self.blob.section(header.flags),
        };
        let tree = DecodedTree::new(&self.decoded);
        (slices, tree)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MeshSlices<'a> {
    vertices: &'a [Vec3],
    triangles: &'a [[u32; 3]],
    materials: &'a [u8],
    flags: &'a [u8],
}

impl MeshSlices<'_> {
    #[inline]
    pub(crate) fn triangle(&self, index: u32) -> Triangle {
        let slot = index as usize;
        let indices = self.triangles[slot];
        Triangle {
            vertices: map3(indices, |vertex| self.vertices[vertex as usize]),
            indices,
            flags: EdgeFlags(self.flags[slot]),
            material: self.materials[slot],
            index,
        }
    }

    #[inline]
    pub(crate) fn vertices(&self, index: u32) -> [Vec3; 3] {
        map3(self.triangles[index as usize], |vertex| {
            self.vertices[vertex as usize]
        })
    }
}
