use crate::draw::arena::Ledger;
use crate::draw::instance::GpuGroupClip;
use crate::scene::Scene;

#[derive(Debug, Default)]
pub(crate) struct GroupTables {
    pub(crate) offsets: Vec<[f32; 2]>,
    pub(crate) offset_ledger: Ledger,
    pub(crate) clips: Vec<GpuGroupClip>,
    pub(crate) clip_ledger: Ledger,
}

impl GroupTables {
    pub(crate) fn open(&mut self) {
        self.offset_ledger.open(self.offsets.len());
        self.clip_ledger.open(self.clips.len());
    }

    pub(crate) fn lay_out(&mut self, scene: &Scene) {
        let slots = scene.group_slots();
        self.offsets.clear();
        self.offsets
            .extend((0..slots).map(|slot| scene.group_offset_at(slot)));
        self.clips.clear();
        self.clips
            .extend((0..slots).map(|slot| GpuGroupClip::resolved(scene, slot)));
        self.offset_ledger.mark_relaid();
        self.clip_ledger.mark_relaid();
    }

    pub(crate) fn refresh(&mut self, scene: &Scene, moved: &[u32], reclipped: &[u32], whole: bool) {
        let slots = scene.group_slots();
        if whole || slots < self.offsets.len() {
            self.lay_out(scene);
            return;
        }
        let from = self.offsets.len();
        if slots > from {
            self.offsets
                .extend((from..slots).map(|slot| scene.group_offset_at(slot)));
            self.clips
                .extend((from..slots).map(|slot| GpuGroupClip::resolved(scene, slot)));
            let (first, count) = (from as u32, (slots - from) as u32);
            self.offset_ledger.note(first, count);
            self.clip_ledger.note(first, count);
        }
        for &slot in moved {
            let at = slot as usize;
            if let Some(entry) = self.offsets.get_mut(at) {
                *entry = scene.group_offset_at(at);
                self.offset_ledger.note(slot, 1);
            }
        }
        for &slot in reclipped {
            let at = slot as usize;
            if let Some(entry) = self.clips.get_mut(at) {
                *entry = GpuGroupClip::resolved(scene, at);
                self.clip_ledger.note(slot, 1);
            }
        }
    }
}
