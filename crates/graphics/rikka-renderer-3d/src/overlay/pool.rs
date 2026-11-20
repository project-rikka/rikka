use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashMap;

use crate::arena::{GpuArena, LaneSpec, Span};
use crate::error::{Error, Resource, Result};
use crate::overlay::batch::{OverlayData, OverlayUpdate, check_indices};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const POSITIONS: usize = 0;
const COLORS: usize = 1;
pub(crate) const POSITION_BYTES: u64 = 12;
pub(crate) const COLOR_BYTES: u64 = 4;
pub(crate) const INDEX_BYTES: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayBufferId(pub(crate) u64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverlayEntry {
    pub vertices: Span,
    pub indices: Span,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Placed {
    pub positions: vk::DeviceAddress,
    pub colors: vk::DeviceAddress,
    pub indices: vk::DeviceAddress,
    pub vertices: u32,
    pub index_count: u32,
}

pub(crate) struct OverlayPool {
    vertices: GpuArena<2>,
    indices: GpuArena<1>,
    buffers: FxHashMap<OverlayBufferId, OverlayEntry>,
    next_id: u64,
}

impl OverlayPool {
    pub(crate) fn new(device: &Arc<RenderDevice>) -> Result<Self> {
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        Ok(Self {
            vertices: GpuArena::new(
                device,
                [
                    LaneSpec {
                        name: "rikka.three.overlay_positions",
                        stride: POSITION_BYTES as u32,
                        usage: storage,
                    },
                    LaneSpec {
                        name: "rikka.three.overlay_colors",
                        stride: COLOR_BYTES as u32,
                        usage: storage,
                    },
                ],
            )?,
            indices: GpuArena::single(
                device,
                "rikka.three.overlay_indices",
                INDEX_BYTES as u32,
                storage,
            )?,
            buffers: FxHashMap::default(),
            next_id: 1,
        })
    }

    pub(crate) fn create(&mut self, data: &OverlayData<'_>) -> Result<OverlayBufferId> {
        data.validate()?;
        if data.positions.is_empty() {
            return Err(Error::Invalid("overlay buffer has no vertices".into()));
        }
        let positions: Vec<[f32; 3]> = data.positions.iter().map(|p| p.to_array()).collect();
        let vertices = self.vertices.insert([
            bytemuck::cast_slice(&positions),
            bytemuck::cast_slice(data.colors),
        ])?;
        let indices = match self.indices.insert([bytemuck::cast_slice(data.indices)]) {
            Ok(indices) => indices,
            Err(error) => {
                self.vertices.cancel(vertices);
                return Err(error);
            }
        };
        let id = OverlayBufferId(self.next_id);
        self.next_id += 1;
        self.buffers.insert(id, OverlayEntry { vertices, indices });
        Ok(id)
    }

    pub(crate) fn update(&mut self, id: OverlayBufferId, update: &OverlayUpdate<'_>) -> Result<()> {
        let entry = self.entry(id)?;
        if !update.positions.is_empty()
            && !update.colors.is_empty()
            && update.positions.len() != update.colors.len()
        {
            return Err(Error::Invalid(
                "overlay update positions and colors differ in length".into(),
            ));
        }
        let positions: Vec<[f32; 3]> = update.positions.iter().map(|p| p.to_array()).collect();
        self.vertices.overwrite(
            POSITIONS,
            entry.vertices,
            update.first,
            bytemuck::cast_slice(&positions),
        )?;
        self.vertices.overwrite(
            COLORS,
            entry.vertices,
            update.first,
            bytemuck::cast_slice(update.colors),
        )
    }

    pub(crate) fn update_indices(
        &mut self,
        id: OverlayBufferId,
        first: u32,
        indices: &[u32],
    ) -> Result<()> {
        let entry = self.entry(id)?;
        check_indices(indices, entry.vertices.count)?;
        self.indices
            .overwrite(0, entry.indices, first, bytemuck::cast_slice(indices))
    }

    pub(crate) fn destroy(&mut self, id: OverlayBufferId) -> bool {
        let Some(entry) = self.buffers.remove(&id) else {
            return false;
        };
        self.vertices.retire(entry.vertices);
        self.indices.retire(entry.indices);
        true
    }

    fn entry(&self, id: OverlayBufferId) -> Result<OverlayEntry> {
        self.buffers
            .get(&id)
            .copied()
            .ok_or(Error::Unknown(Resource::OverlayBuffer(id)))
    }

    pub(crate) fn placed(&self, id: OverlayBufferId) -> Result<Placed> {
        let entry = self.entry(id)?;
        let indices = match entry.indices.count {
            0 => 0,
            _ => self.indices.address() + u64::from(entry.indices.offset) * INDEX_BYTES,
        };
        Ok(Placed {
            positions: self.vertices.lane(POSITIONS).address()
                + u64::from(entry.vertices.offset) * POSITION_BYTES,
            colors: self.vertices.lane(COLORS).address()
                + u64::from(entry.vertices.offset) * COLOR_BYTES,
            indices,
            vertices: entry.vertices.count,
            index_count: entry.indices.count,
        })
    }

    pub(crate) fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        self.vertices.flush(ctx)?;
        self.indices.flush(ctx)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.vertices.has_pending() || self.indices.has_pending()
    }
}
