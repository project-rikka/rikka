use std::hash::{Hash, Hasher};

use glam::Vec3;
use rustc_hash::FxHasher;
use smallvec::SmallVec;

use crate::gpu::{GpuInstance, NONE_INDEX, instance_flags};
use crate::light::{Light, Placed};
use crate::shadow::ShadowView;

#[derive(Clone, Debug)]
struct Entry {
    key: u64,
    casters: u64,
    tile: u32,
    origins: SmallVec<[(u32, u32); 6]>,
}

#[derive(Debug, Default)]
pub(crate) struct ShadowCache {
    entries: Vec<Entry>,
    next: Vec<Entry>,
    atlas: u64,
}

pub(crate) struct Reuse {
    pub key: u64,
    pub casters: Option<u64>,
}

impl ShadowCache {
    pub(crate) fn begin(&mut self, atlas: u64) {
        if self.atlas != atlas {
            self.entries.clear();
            self.atlas = atlas;
        }
        self.next.clear();
    }

    pub(crate) fn cached(&self, reuse: &Reuse, tile: u32) -> Option<&[(u32, u32)]> {
        let casters = reuse.casters?;
        self.entries
            .iter()
            .find(|entry| entry.key == reuse.key && entry.casters == casters && entry.tile == tile)
            .map(|entry| entry.origins.as_slice())
    }

    pub(crate) fn keep(&mut self, reuse: &Reuse, tile: u32, views: &[ShadowView]) {
        let Some(casters) = reuse.casters else {
            return;
        };
        self.next.push(Entry {
            key: reuse.key,
            casters,
            tile,
            origins: views.iter().map(|view| view.origin).collect(),
        });
    }

    pub(crate) fn finish(&mut self) {
        std::mem::swap(&mut self.entries, &mut self.next);
    }

    pub(crate) fn forget(&mut self) {
        self.entries.clear();
        self.next.clear();
    }
}

pub(crate) fn light_key(light: &Light, tile: u32) -> u64 {
    let mut hasher = FxHasher::default();
    bytemuck::bytes_of(&light.to_gpu(Placed::default(), NONE_INDEX)).hash(&mut hasher);
    light.shadow.hash(&mut hasher);
    tile.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn casters(light: &Light, instances: &[GpuInstance]) -> Option<u64> {
    let center = light.kind.position();
    let reach = light.kind.range() + light.kind.extent();
    let mut hasher = FxHasher::default();
    for instance in instances {
        if instance.index_count == 0
            || instance.flags & instance_flags::CASTS_SHADOW == 0
            || instance.light_channels & light.channels == 0
        {
            continue;
        }
        let apart = Vec3::from_array(instance.center).distance(center);
        if apart > reach + instance.radius {
            continue;
        }
        if instance.flags & instance_flags::DEFORMED != 0 || !instance.radius.is_finite() {
            return None;
        }
        bytemuck::bytes_of(&instance.transform).hash(&mut hasher);
        (
            instance.positions,
            instance.vertex_offset,
            instance.first_index,
            instance.index_count,
            instance.material,
            instance.shadow_class,
            instance.flags,
        )
            .hash(&mut hasher);
    }
    Some(hasher.finish())
}
