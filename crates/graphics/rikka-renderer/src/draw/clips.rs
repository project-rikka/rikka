use rikka_types::RoundedRect;

use crate::draw::arena::Ledger;
use crate::draw::instance::{ClipSlots, GpuGroupClip};
use crate::scene::Node;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HeldClips {
    clip: Option<u32>,
    cut: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct ClipPool {
    pub(crate) entries: Vec<GpuGroupClip>,
    free: Vec<u32>,
    pub(crate) ledger: Ledger,
}

impl Default for ClipPool {
    fn default() -> Self {
        Self {
            entries: vec![GpuGroupClip::UNCLIPPED],
            free: Vec::new(),
            ledger: Ledger::default(),
        }
    }
}

impl ClipPool {
    pub(crate) fn open(&mut self) {
        self.ledger.open(self.entries.len());
    }

    pub(crate) fn reset(&mut self) {
        self.entries.truncate(1);
        self.free.clear();
        self.ledger.mark_relaid();
    }

    pub(crate) fn hold(&mut self, held: &mut HeldClips, node: &Node) -> ClipSlots {
        ClipSlots {
            clip: self.put(&mut held.clip, node.clip),
            cut: self.put(&mut held.cut, node.cutout.as_deref().copied()),
        }
    }

    pub(crate) fn release(&mut self, held: &mut HeldClips) {
        self.free.extend(held.clip.take());
        self.free.extend(held.cut.take());
    }

    pub(crate) fn push(&mut self, node: &Node) -> ClipSlots {
        ClipSlots {
            clip: self.append(node.clip),
            cut: self.append(node.cutout.as_deref().copied()),
        }
    }

    fn put(&mut self, slot: &mut Option<u32>, shape: Option<RoundedRect>) -> u32 {
        let Some(shape) = shape else {
            self.free.extend(slot.take());
            return 0;
        };
        let value = GpuGroupClip::from_rounded(shape);
        if let Some(index) = *slot {
            if self.entries[index as usize] != value {
                self.entries[index as usize] = value;
                self.ledger.note(index, 1);
            }
            return index;
        }
        let index = match self.free.pop() {
            Some(index) => {
                self.entries[index as usize] = value;
                index
            }
            None => {
                let index = self.entries.len() as u32;
                self.entries.push(value);
                index
            }
        };
        self.ledger.note(index, 1);
        *slot = Some(index);
        index
    }

    fn append(&mut self, shape: Option<RoundedRect>) -> u32 {
        let Some(shape) = shape else {
            return 0;
        };
        let index = self.entries.len() as u32;
        self.entries.push(GpuGroupClip::from_rounded(shape));
        index
    }
}
