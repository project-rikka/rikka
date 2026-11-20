use std::sync::Arc;

use ash::vk;
use rikka_ies::IesProfile;

use crate::error::{Error, Result};
use crate::light::profile::{LightProfileId, PROFILE_TEXELS, bake};
use crate::mirror::MirroredTable;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

struct Slot {
    generation: u32,
    live: bool,
}

pub(crate) struct LightProfilePool {
    table: MirroredTable<f32, PROFILE_TEXELS>,
    slots: Vec<Slot>,
}

impl LightProfilePool {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Self {
        Self {
            table: MirroredTable::new(
                device,
                "rikka.three.light_profiles",
                vk::PipelineStageFlags2::FRAGMENT_SHADER | vk::PipelineStageFlags2::COMPUTE_SHADER,
            ),
            slots: Vec::new(),
        }
    }

    pub(crate) fn create(&mut self, profile: &IesProfile, finished: u64) -> Result<LightProfileId> {
        if profile.peak_candela() <= 0.0 {
            return Err(Error::Invalid("light profile emits no light".into()));
        }
        self.table.reclaim(finished);
        let index = self.table.insert(|texels| bake(profile, texels));
        if index as usize == self.slots.len() {
            self.slots.push(Slot {
                generation: 0,
                live: false,
            });
        }
        let slot = &mut self.slots[index as usize];
        slot.live = true;
        Ok(LightProfileId {
            index,
            generation: slot.generation,
        })
    }

    pub(crate) fn destroy(&mut self, id: LightProfileId, after: u64) -> bool {
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return false;
        };
        if !slot.live || slot.generation != id.generation {
            return false;
        }
        slot.live = false;
        slot.generation = slot.generation.wrapping_add(1);
        self.table.retire(id.index, after);
        true
    }

    pub(crate) fn slot(&self, id: LightProfileId) -> Option<u32> {
        self.slots
            .get(id.index as usize)
            .filter(|slot| slot.live && slot.generation == id.generation)
            .map(|_| id.index)
    }

    pub(crate) fn address(&self) -> vk::DeviceAddress {
        self.table.address()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.table.has_pending()
    }

    pub(crate) fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        self.table.flush(ctx)
    }
}
