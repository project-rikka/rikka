use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use rikka_physics_core::{Blob, BlobWriter, Section};
use rikka_physics_math::{Aabb, Transform, Vec3};

use super::record::{BlobRecord, ChildRecord, HULL, MESH};
use crate::triangulated::bvh::{Bvh4, QuantizedNode4, QuantizedTree};
use crate::triangulated::mesh::{MeshShape, TriangleMesh};
use crate::{ConvexHull, Error, Result, Shape, ShapeGeometry};

const LEAF_SIZE: u32 = 1;
const BLOB_ALIGN: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct CompoundHeader {
    pub(crate) bounds: Aabb,
    pub(crate) nodes: Section<QuantizedNode4>,
    pub(crate) order: Section<u32>,
    pub(crate) children: Section<ChildRecord>,
    pub(crate) blobs: Section<BlobRecord>,
    pub(crate) bytes: Section<u8>,
}

#[derive(Clone, Debug)]
pub struct CompoundChild {
    pub shape: Shape,
    pub transform: Transform,
    pub material: u32,
}

impl CompoundChild {
    pub fn new(shape: impl Into<Shape>, transform: Transform) -> Self {
        Self {
            shape: shape.into(),
            transform,
            material: 0,
        }
    }

    #[must_use]
    pub fn material(mut self, material: u32) -> Self {
        self.material = material;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChildRef<'a> {
    pub shape: &'a Shape,
    pub transform: Transform,
    pub material: u32,
}

#[derive(Clone, Debug)]
pub struct Compound {
    blob: Blob<CompoundHeader>,
    shapes: Arc<[Shape]>,
}

impl PartialEq for Compound {
    fn eq(&self, other: &Self) -> bool {
        self.blob == other.blob
    }
}

#[derive(Default)]
struct Embedded {
    records: Vec<BlobRecord>,
    bytes: Vec<u8>,
    hashes: Vec<(u64, u32)>,
}

impl Embedded {
    fn add(&mut self, hash: u64, bytes: &[u8]) -> u32 {
        for &(known, index) in &self.hashes {
            let record = self.records[index as usize];
            let start = record.offset as usize;
            if known == hash && &self.bytes[start..start + record.len as usize] == bytes {
                return index;
            }
        }
        let offset = self.bytes.len();
        self.bytes.extend_from_slice(bytes);
        self.bytes
            .resize(self.bytes.len().next_multiple_of(BLOB_ALIGN), 0);
        let index = self.records.len() as u32;
        self.records.push(BlobRecord {
            offset: offset as u32,
            len: bytes.len() as u32,
        });
        self.hashes.push((hash, index));
        index
    }
}

fn check_transform(transform: Transform) -> Result<()> {
    let length = transform.q.length();
    if !transform.is_finite() {
        return Err(Error::NonFinite {
            what: "compound child transform",
        });
    }
    if (length - 1.0).abs() > 1.0e-3 {
        return Err(Error::UnnormalizedRotation {
            what: "compound child transform",
            length,
        });
    }
    Ok(())
}

impl Compound {
    pub fn build(children: &[CompoundChild]) -> Result<Self> {
        if children.is_empty() {
            return Err(Error::EmptyCompound);
        }
        let mut records = Vec::with_capacity(children.len());
        let mut embedded = Embedded::default();
        for child in children {
            check_transform(child.transform)?;
            child.shape.validate()?;
            let record = match &child.shape {
                Shape::Hull(hull) => {
                    let mut record = ChildRecord {
                        kind: HULL,
                        blob: embedded.add(hull.content_hash(), hull.blob().bytes()),
                        ..blank(child)
                    };
                    record.params[0] = hull.radius();
                    record
                }
                Shape::Mesh(mesh) => {
                    let mut record = ChildRecord {
                        kind: MESH,
                        blob: embedded.add(mesh.mesh.content_hash(), mesh.mesh.bytes()),
                        ..blank(child)
                    };
                    record.params[..3].copy_from_slice(&mesh.scale.to_array());
                    record
                }
                shape => ChildRecord::value(child.transform, child.material, shape)
                    .ok_or(Error::UnsupportedChild { kind: shape.kind() })?,
            };
            records.push(record);
        }
        let bounds: Vec<Aabb> = children
            .iter()
            .map(|child| child.shape.aabb(child.transform))
            .collect();
        let tree = Bvh4::builder()
            .leaf_size(LEAF_SIZE)
            .build(&bounds, 0..children.len() as u32);
        let quantized = tree.quantize();
        let mut writer = BlobWriter::<CompoundHeader>::new();
        let nodes = writer.push(&quantized.nodes);
        let order = writer.push(tree.items());
        let children_section = writer.push(&records);
        let blobs = writer.push(&embedded.records);
        let bytes = writer.push(&embedded.bytes);
        let blob = writer.finish(CompoundHeader {
            bounds: quantized.bounds,
            nodes,
            order,
            children: children_section,
            blobs,
            bytes,
        });
        Ok(Self {
            blob,
            shapes: children.iter().map(|child| child.shape.clone()).collect(),
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let blob = Blob::<CompoundHeader>::from_bytes(bytes)?;
        let header = *blob.header();
        let corrupt = || Error::CorruptShape { shape: "compound" };
        let records = blob.try_section(header.children)?;
        let order = blob.try_section(header.order)?;
        let nodes = blob.try_section(header.nodes)?;
        let blobs = blob.try_section(header.blobs)?;
        let embedded = blob.try_section(header.bytes)?;
        if records.is_empty()
            || order.len() != records.len()
            || order.iter().any(|&child| child as usize >= records.len())
            || !QuantizedTree::new(nodes, header.bounds).is_consistent(order.len() as u32)
        {
            return Err(corrupt());
        }
        let mut decoded: Vec<Option<Shape>> = vec![None; blobs.len()];
        let mut shapes = Vec::with_capacity(records.len());
        for record in records {
            check_transform(record.transform())?;
            let shape = if record.kind == HULL || record.kind == MESH {
                let index = record.blob as usize;
                let entry = blobs.get(index).ok_or_else(corrupt)?;
                let start = entry.offset as usize;
                let bytes = embedded
                    .get(start..start + entry.len as usize)
                    .ok_or_else(corrupt)?;
                let base = match &decoded[index] {
                    Some(shape) => shape.clone(),
                    None => {
                        let shape = if record.kind == HULL {
                            Shape::Hull(ConvexHull::from_blob(Blob::from_bytes(bytes)?)?)
                        } else {
                            Shape::Mesh(MeshShape::new(TriangleMesh::from_bytes(bytes)?))
                        };
                        decoded[index] = Some(shape.clone());
                        shape
                    }
                };
                match base {
                    Shape::Mesh(mesh) => {
                        let p = &record.params;
                        Shape::Mesh(MeshShape::scaled(mesh.mesh, Vec3::new(p[0], p[1], p[2])))
                    }
                    Shape::Hull(hull) => Shape::Hull(hull.rounded(record.params[0])),
                    other => other,
                }
            } else {
                record.decode_value().ok_or_else(corrupt)?
            };
            shape.validate()?;
            shapes.push(shape);
        }
        Ok(Self {
            blob,
            shapes: shapes.into(),
        })
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
    pub fn bounds(&self) -> Aabb {
        self.blob.header().bounds
    }

    #[inline]
    pub fn child_count(&self) -> u32 {
        self.shapes.len() as u32
    }

    #[inline]
    pub fn child(&self, index: u32) -> ChildRef<'_> {
        let record = &self.records()[index as usize];
        ChildRef {
            shape: &self.shapes[index as usize],
            transform: record.transform(),
            material: record.material,
        }
    }

    #[inline]
    pub fn children(&self) -> impl ExactSizeIterator<Item = ChildRef<'_>> + '_ {
        (0..self.child_count()).map(|index| self.child(index))
    }

    #[inline]
    fn records(&self) -> &[ChildRecord] {
        self.blob.section(self.blob.header().children)
    }

    #[inline]
    pub(crate) fn order(&self) -> &[u32] {
        self.blob.section(self.blob.header().order)
    }

    #[inline]
    pub fn tree(&self) -> QuantizedTree<'_> {
        QuantizedTree::new(self.blob.section(self.blob.header().nodes), self.bounds())
    }

    pub fn query_children(&self, bounds: Aabb, mut visit: impl FnMut(u32) -> bool) -> bool {
        let order = self.order();
        self.tree()
            .query(bounds, |slot| visit(order[slot as usize]))
    }
}

fn blank(child: &CompoundChild) -> ChildRecord {
    ChildRecord {
        position: child.transform.p,
        rotation: child.transform.q.to_array(),
        material: child.material,
        blob: u32::MAX,
        ..ChildRecord::default()
    }
}
