use crate::draw::arena::Ledger;
use crate::draw::chunks::{Chunk, Segment};
use crate::draw::instance::{GpuGroupClip, GpuInstance, GpuPaint};
use crate::draw::retained::RetainedBatch;
use crate::scene::SceneId;

pub(crate) struct Batch<'a> {
    pub chunks: &'a [Chunk],
    pub segments: &'a [Segment],
    pub textured: &'a [u32],
    pub retained: Option<&'a RetainedBatch>,
    pub upload: Upload<'a>,
}

pub(crate) struct Upload<'a> {
    pub scene: Option<SceneId>,
    pub instances: Ledgered<'a, GpuInstance>,
    pub groups: Ledgered<'a, [f32; 2]>,
    pub clips: Ledgered<'a, GpuGroupClip>,
    pub shown: Ledgered<'a, u32>,
    pub captured: Ledgered<'a, u32>,
    pub moves: Ledgered<'a, [f32; 2]>,
    pub extras: Ledgered<'a, GpuGroupClip>,
    pub paints: Ledgered<'a, GpuPaint>,
}

pub(crate) struct Ledgered<'a, T> {
    pub data: &'a [T],
    pub ledger: &'a Ledger,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Work {
    pub walked: usize,
    pub emitted: usize,
    pub reused: usize,
    pub restructured: bool,
}
