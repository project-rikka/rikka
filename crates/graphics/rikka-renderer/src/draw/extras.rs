use crate::draw::arena::Ledger;
use crate::draw::instance::{ExtraSlots, GpuGroupClip, GpuPaint};
use crate::scene::Node;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HeldExtras {
    clip: Option<u32>,
    cut: Option<u32>,
    paint: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct Table<T> {
    pub(crate) entries: Vec<T>,
    free: Vec<u32>,
    pub(crate) ledger: Ledger,
}

impl<T: Copy + PartialEq> Table<T> {
    fn new(unused: T) -> Self {
        Self {
            entries: vec![unused],
            free: Vec::new(),
            ledger: Ledger::default(),
        }
    }

    fn open(&mut self) {
        self.ledger.open(self.entries.len());
    }

    fn reset(&mut self) {
        self.entries.truncate(1);
        self.free.clear();
        self.ledger.mark_relaid();
    }

    fn release(&mut self, slot: &mut Option<u32>) {
        self.free.extend(slot.take());
    }

    fn put(&mut self, slot: &mut Option<u32>, value: Option<T>) -> u32 {
        let Some(value) = value else {
            self.release(slot);
            return 0;
        };
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

    fn append(&mut self, value: Option<T>) -> u32 {
        let Some(value) = value else {
            return 0;
        };
        let index = self.entries.len() as u32;
        self.entries.push(value);
        index
    }
}

#[derive(Debug)]
pub(crate) struct ExtraPool {
    pub(crate) clips: Table<GpuGroupClip>,
    pub(crate) paints: Table<GpuPaint>,
}

impl Default for ExtraPool {
    fn default() -> Self {
        Self {
            clips: Table::new(GpuGroupClip::UNCLIPPED),
            paints: Table::new(GpuPaint::UNPAINTED),
        }
    }
}

impl ExtraPool {
    pub(crate) fn open(&mut self) {
        self.clips.open();
        self.paints.open();
    }

    pub(crate) fn reset(&mut self) {
        self.clips.reset();
        self.paints.reset();
    }

    pub(crate) fn hold(&mut self, held: &mut HeldExtras, node: &Node) -> ExtraSlots {
        ExtraSlots {
            clip: self.clips.put(&mut held.clip, clip_of(node)),
            cut: self.clips.put(&mut held.cut, cut_of(node)),
            paint: self.paints.put(&mut held.paint, GpuPaint::of(node)),
        }
    }

    pub(crate) fn release(&mut self, held: &mut HeldExtras) {
        self.clips.release(&mut held.clip);
        self.clips.release(&mut held.cut);
        self.paints.release(&mut held.paint);
    }

    pub(crate) fn push(&mut self, node: &Node) -> ExtraSlots {
        ExtraSlots {
            clip: self.clips.append(clip_of(node)),
            cut: self.clips.append(cut_of(node)),
            paint: self.paints.append(GpuPaint::of(node)),
        }
    }
}

fn clip_of(node: &Node) -> Option<GpuGroupClip> {
    node.clip.map(GpuGroupClip::from_rounded)
}

fn cut_of(node: &Node) -> Option<GpuGroupClip> {
    node.cutout
        .as_deref()
        .copied()
        .map(GpuGroupClip::from_rounded)
}
