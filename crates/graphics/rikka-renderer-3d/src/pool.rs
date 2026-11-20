use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashMap;

use crate::arena::{GpuArena, LaneSpec, Span};
use crate::attribute_writes::AttributeWrites;
use crate::error::{Error, Resource, Result};
use crate::mesh::{
    IndexWidth, Indices, MeshBuild, MeshData, MeshEntry, MeshId, MeshUpdate, MorphEntry, Shape,
    Submesh, Topology, build, topology,
};
use crate::pipeline::MeshPipelines;
use crate::reach::{self, Geometry, Morphs};
use crate::vertex::{GpuAttribute, GpuMorphDelta, GpuPosition, GpuSkin};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const POSITIONS: usize = 0;
const ATTRIBUTES: usize = 1;

pub(crate) struct MeshPool {
    vertices: GpuArena<2>,
    pub skins: GpuArena<1>,
    indices16: GpuArena<1>,
    indices32: GpuArena<1>,
    pub morph_starts: GpuArena<1>,
    pub morph_deltas: GpuArena<1>,
    pub outline_normals: GpuArena<1>,
    pub edges: GpuArena<1>,
    attribute_writes: AttributeWrites,
    meshes: FxHashMap<MeshId, MeshEntry>,
    next_id: u64,
    revision: u64,
}

impl MeshPool {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let build_input = traced_input(&device);
        Ok(Self {
            vertices: GpuArena::new(
                &device,
                [
                    LaneSpec {
                        name: "rikka.three.positions",
                        stride: size_of::<GpuPosition>() as u32,
                        usage: storage | build_input,
                    },
                    LaneSpec {
                        name: "rikka.three.attributes",
                        stride: size_of::<GpuAttribute>() as u32,
                        usage: storage,
                    },
                ],
            )?,
            skins: GpuArena::single(
                &device,
                "rikka.three.skins",
                size_of::<GpuSkin>() as u32,
                storage,
            )?,
            indices16: GpuArena::single(
                &device,
                "rikka.three.indices16",
                2,
                vk::BufferUsageFlags::INDEX_BUFFER | build_input,
            )?,
            indices32: GpuArena::single(
                &device,
                "rikka.three.indices32",
                4,
                vk::BufferUsageFlags::INDEX_BUFFER | build_input,
            )?,
            morph_starts: GpuArena::single(&device, "rikka.three.morph_starts", 4, storage)?,
            morph_deltas: GpuArena::single(
                &device,
                "rikka.three.morph_deltas",
                size_of::<GpuMorphDelta>() as u32,
                storage,
            )?,
            outline_normals: GpuArena::single(&device, "rikka.three.outline_normals", 4, storage)?,
            edges: GpuArena::single(
                &device,
                "rikka.three.edges",
                size_of::<[u32; 4]>() as u32,
                storage,
            )?,
            attribute_writes: AttributeWrites::new(device),
            meshes: FxHashMap::default(),
            next_id: 1,
            revision: 0,
        })
    }

    pub(crate) fn positions_address(&self) -> vk::DeviceAddress {
        self.vertices.lane(POSITIONS).address()
    }

    pub(crate) fn attributes_address(&self) -> vk::DeviceAddress {
        self.vertices.lane(ATTRIBUTES).address()
    }

    pub(crate) fn index_address(&self, width: IndexWidth) -> vk::DeviceAddress {
        self.index_arena(width).address()
    }

    pub(crate) fn index_buffer(&self, width: IndexWidth) -> vk::Buffer {
        self.index_arena(width).handle()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    fn index_arena(&self, width: IndexWidth) -> &GpuArena<1> {
        match width {
            IndexWidth::U16 => &self.indices16,
            IndexWidth::U32 => &self.indices32,
        }
    }

    fn index_arena_mut(&mut self, width: IndexWidth) -> &mut GpuArena<1> {
        match width {
            IndexWidth::U16 => &mut self.indices16,
            IndexWidth::U32 => &mut self.indices32,
        }
    }

    pub(crate) fn create(&mut self, data: &MeshData<'_>) -> Result<MeshId> {
        let built = build(data)?;
        let entry = self.place(built)?;
        let id = MeshId(self.next_id);
        self.next_id += 1;
        self.meshes.insert(id, entry);
        Ok(id)
    }

    pub(crate) fn replace(&mut self, id: MeshId, data: &MeshData<'_>) -> Result<()> {
        if !self.meshes.contains_key(&id) {
            return Err(Error::Unknown(Resource::Mesh(id)));
        }
        let built = build(data)?;
        let entry = self.place(built)?;
        let previous = self
            .meshes
            .insert(id, entry)
            .expect("the replaced mesh exists");
        self.retire(previous);
        self.revision += 1;
        Ok(())
    }

    pub(crate) fn update(&mut self, id: MeshId, update: &MeshUpdate<'_>) -> Result<()> {
        let Some(entry) = self.meshes.get_mut(&id) else {
            return Err(Error::Unknown(Resource::Mesh(id)));
        };
        let count = update.count(entry.vertex_count())?;
        if count == 0 {
            return Ok(());
        }
        if !update.positions.is_empty() {
            let positions: Vec<GpuPosition> = update
                .positions
                .iter()
                .map(|position| GpuPosition {
                    position: position.to_array(),
                })
                .collect();
            self.vertices.overwrite(
                POSITIONS,
                entry.vertices,
                update.first,
                bytemuck::cast_slice(&positions),
            )?;
            let first = update.first as usize;
            entry.shape.positions[first..first + count as usize].copy_from_slice(update.positions);
            let touched = update.first..update.first + count;
            for part in &mut entry.parts {
                if part.vertices.start < touched.end && touched.start < part.vertices.end {
                    part.refit(&entry.shape.positions, &entry.shape.skins, entry.joints);
                }
            }
        }
        self.attribute_writes
            .push(entry.vertices.offset + update.first, update, count);
        self.revision += 1;
        Ok(())
    }

    pub(crate) fn replace_indices(
        &mut self,
        id: MeshId,
        indices: Indices<'_>,
        submeshes: &[Submesh],
    ) -> Result<()> {
        let Some(entry) = self.meshes.get(&id) else {
            return Err(Error::Unknown(Resource::Mesh(id)));
        };
        let Topology {
            indices,
            index_width,
            submeshes,
        } = topology(Some(indices), submeshes, &entry.shape.positions)?;
        let morphs = reach::morph_bound(&entry.parts);
        let parts = reach::parts(
            submeshes,
            &Geometry {
                positions: &entry.shape.positions,
                indices: &indices,
                skins: &entry.shape.skins,
                joints: entry.joints,
                morphs: Morphs::Bounded(&morphs),
            },
        );
        let span = self.insert_indices(index_width, &indices)?;
        let entry = self.meshes.get_mut(&id).expect("the mesh was just found");
        let previous = std::mem::replace(&mut entry.indices, span);
        let previous_width = std::mem::replace(&mut entry.index_width, index_width);
        entry.parts = parts;
        let edges = std::mem::take(&mut entry.edges);
        self.index_arena_mut(previous_width).retire(previous);
        for span in edges.into_iter().flatten() {
            self.edges.retire(span);
        }
        self.revision += 1;
        Ok(())
    }

    pub(crate) fn set_edges(&mut self, id: MeshId, submesh: u32, edges: &[[u32; 4]]) -> Result<()> {
        let Some(entry) = self.meshes.get(&id) else {
            return Err(Error::Unknown(Resource::Mesh(id)));
        };
        let parts = entry.parts.len();
        if submesh as usize >= parts {
            return Err(Error::Invalid(format!(
                "edge submesh {submesh} is out of range for {parts} submeshes"
            )));
        }
        let vertices = entry.vertex_count();
        if let Some(worst) = edges.iter().flatten().copied().max()
            && worst >= vertices
        {
            return Err(Error::Invalid(format!(
                "edge vertex {worst} is out of range for {vertices} vertices"
            )));
        }
        let span = self.edges.insert([bytemuck::cast_slice(edges)])?;
        let entry = self.meshes.get_mut(&id).expect("the mesh was just found");
        if entry.edges.len() < parts {
            entry.edges.resize(parts, None);
        }
        if let Some(previous) = entry.edges[submesh as usize].replace(span) {
            self.edges.retire(previous);
        }
        Ok(())
    }

    fn insert_indices(&mut self, width: IndexWidth, indices: &[u32]) -> Result<Span> {
        match width {
            IndexWidth::U16 => {
                let narrow: Vec<u16> = indices.iter().map(|&index| index as u16).collect();
                self.indices16.insert([bytemuck::cast_slice(&narrow)])
            }
            IndexWidth::U32 => self.indices32.insert([bytemuck::cast_slice(indices)]),
        }
    }

    fn place(&mut self, built: MeshBuild) -> Result<MeshEntry> {
        let mut placed = MeshSpans::default();
        let entry = self.place_into(built, &mut placed);
        if entry.is_err() {
            self.cancel(placed);
        }
        entry
    }

    fn place_into(&mut self, built: MeshBuild, placed: &mut MeshSpans) -> Result<MeshEntry> {
        placed.vertices = self.vertices.insert([
            bytemuck::cast_slice(&built.positions),
            bytemuck::cast_slice(&built.attributes),
        ])?;
        placed.skins = self.skins.insert([bytemuck::cast_slice(&built.skins)])?;
        placed.index_width = built.index_width;
        placed.indices = self.insert_indices(built.index_width, &built.indices)?;
        placed.morph_starts = self
            .morph_starts
            .insert([bytemuck::cast_slice(&built.morph.starts)])?;
        placed.morph_deltas = self
            .morph_deltas
            .insert([bytemuck::cast_slice(&built.morph.deltas)])?;
        placed.outline_normals = self
            .outline_normals
            .insert([bytemuck::cast_slice(&built.outline_normals)])?;
        Ok(MeshEntry {
            vertices: placed.vertices,
            skins: placed.skins,
            indices: placed.indices,
            index_width: built.index_width,
            parts: built.parts,
            morph: MorphEntry {
                starts: placed.morph_starts,
                deltas: placed.morph_deltas,
                targets: built.morph.targets,
            },
            joints: built.joints,
            outline_normals: placed.outline_normals,
            edges: Vec::new(),
            shape: Shape {
                positions: built.shape,
                skins: built.skins,
            },
        })
    }

    fn cancel(&mut self, placed: MeshSpans) {
        self.vertices.cancel(placed.vertices);
        self.skins.cancel(placed.skins);
        self.index_arena_mut(placed.index_width)
            .cancel(placed.indices);
        self.morph_starts.cancel(placed.morph_starts);
        self.morph_deltas.cancel(placed.morph_deltas);
        self.outline_normals.cancel(placed.outline_normals);
    }

    pub(crate) fn get(&self, id: MeshId) -> Option<&MeshEntry> {
        self.meshes.get(&id)
    }

    pub(crate) fn destroy(&mut self, id: MeshId) -> bool {
        let Some(entry) = self.meshes.remove(&id) else {
            return false;
        };
        self.retire(entry);
        true
    }

    fn retire(&mut self, entry: MeshEntry) {
        self.vertices.retire(entry.vertices);
        self.skins.retire(entry.skins);
        self.index_arena_mut(entry.index_width)
            .retire(entry.indices);
        self.morph_starts.retire(entry.morph.starts);
        self.morph_deltas.retire(entry.morph.deltas);
        self.outline_normals.retire(entry.outline_normals);
        for span in entry.edges.into_iter().flatten() {
            self.edges.retire(span);
        }
    }

    pub(crate) fn flush(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        pipelines: &mut MeshPipelines,
    ) -> Result<()> {
        self.vertices.flush(ctx)?;
        self.skins.flush(ctx)?;
        self.indices16.flush(ctx)?;
        self.indices32.flush(ctx)?;
        self.morph_starts.flush(ctx)?;
        self.morph_deltas.flush(ctx)?;
        self.outline_normals.flush(ctx)?;
        self.edges.flush(ctx)?;
        let attributes = self.attributes_address();
        self.attribute_writes.flush(ctx, pipelines, attributes)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.vertices.has_pending()
            || self.skins.has_pending()
            || self.indices16.has_pending()
            || self.indices32.has_pending()
            || self.morph_starts.has_pending()
            || self.morph_deltas.has_pending()
            || self.outline_normals.has_pending()
            || self.edges.has_pending()
            || self.attribute_writes.has_pending()
    }
}

#[derive(Default)]
struct MeshSpans {
    vertices: Span,
    skins: Span,
    indices: Span,
    index_width: IndexWidth,
    morph_starts: Span,
    morph_deltas: Span,
    outline_normals: Span,
}

pub(crate) fn traced_input(device: &RenderDevice) -> vk::BufferUsageFlags {
    match device.ray_tracing() {
        Some(_) => vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        None => vk::BufferUsageFlags::empty(),
    }
}
