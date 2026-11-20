mod context;
mod finish;
mod general;
mod pools;
mod primitives;
mod recycle;
mod triangulated;

use rikka_physics_core::{BitSet, Block, Executor, PerWorker, Shared, Stage, prefetch};
use rikka_physics_geometry::Tolerance;
use rikka_physics_math::{Aabb, Kernel, Simd, Wide};

use crate::contact::{
    ContactGeometry, ContactHeader, DISJOINT, PairKind, STOPPED_TOUCHING, TOUCHING,
};
use crate::{BodyFrame, Collider, worker_reserve};
use context::KernelContext;
pub use pools::{ManifoldPools, PoolView};
use triangulated::TriangulatedScratch;

const CLASSIFY_BLOCK: (usize, usize) = (32, 128);
const KERNEL_CHUNKS: usize = 4;
const TRIANGULATED_BLOCK: usize = 4;
const MAX_TRIANGULATED_BLOCK: usize = 32;
const BLOCKS_PER_WORKER: usize = 8;
const PREFETCH_DISTANCE: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct CollideInput<'a> {
    pub colliders: &'a [Collider],
    pub frames: &'a [BodyFrame],
    pub fat: &'a [Aabb],
    pub awake: &'a [u32],
    pub tolerance: Tolerance,
    pub recycle: bool,
}

#[derive(Debug)]
pub struct Narrowphase {
    buckets: PerWorker<[Vec<u32>; PairKind::COUNT]>,
    changed: PerWorker<BitSet>,
    lists: [Vec<u32>; PairKind::COUNT],
    merged: BitSet,
    changes: Vec<u32>,
    pools: ManifoldPools,
    scratch: PerWorker<TriangulatedScratch>,
    previous: Vec<PoolView>,
}

impl Default for Narrowphase {
    fn default() -> Self {
        Self::new()
    }
}

struct CollideBlock<'a, 'b> {
    kind: PairKind,
    window: &'a [u32],
    count: usize,
    ctx: &'a KernelContext<'b>,
    changed: &'a mut BitSet,
    worker: u32,
}

impl Kernel for CollideBlock<'_, '_> {
    type Output = ();

    #[inline(always)]
    fn run<W: Wide>(self) {
        let (ctx, changed) = (self.ctx, self.changed);
        let ids = &self.window[..self.count];
        match self.kind {
            PairKind::Triangulated => {
                for (index, &id) in ids.iter().enumerate() {
                    if let Some(&ahead) = self.window.get(index + PREFETCH_DISTANCE) {
                        ctx.headers.prefetch(ahead as usize, usize::MAX);
                        ctx.contacts.prefetch(ahead as usize, usize::MAX);
                    }
                    triangulated::collide_one(id, ctx, self.worker, changed);
                }
            }
            kind => {
                for (index, chunk) in ids.chunks(W::LANES).enumerate() {
                    let next = (index + 1) * W::LANES;
                    for &id in self.window.iter().skip(next).take(W::LANES) {
                        ctx.headers.prefetch(id as usize, usize::MAX);
                        ctx.contacts
                            .prefetch(id as usize, ContactGeometry::recycle_bytes(4));
                    }
                    match kind {
                        PairKind::SphereSphere => {
                            primitives::sphere_sphere::<W>(chunk, ctx, changed)
                        }
                        PairKind::SphereCapsule => {
                            primitives::sphere_capsule::<W>(chunk, ctx, changed)
                        }
                        PairKind::CapsuleCapsule => {
                            primitives::capsule_capsule::<W>(chunk, ctx, changed)
                        }
                        _ => primitives::sphere_cuboid::<W>(chunk, ctx, changed),
                    }
                }
            }
        }
    }
}

impl Narrowphase {
    pub fn new() -> Self {
        Self {
            buckets: PerWorker::new(1, |_| Default::default()),
            changed: PerWorker::new(1, |_| BitSet::default()),
            lists: Default::default(),
            merged: BitSet::default(),
            changes: Vec::new(),
            pools: ManifoldPools::default(),
            scratch: PerWorker::new(1, |_| TriangulatedScratch::new(Tolerance::default())),
            previous: Vec::new(),
        }
    }

    pub fn stamp(&self) -> u32 {
        self.pools.stamp()
    }

    pub fn pools(&self) -> &ManifoldPools {
        &self.pools
    }

    pub fn pools_mut(&mut self) -> &mut ManifoldPools {
        &mut self.pools
    }

    pub fn collide(
        &mut self,
        executor: &dyn Executor,
        simd: Simd,
        input: &CollideInput<'_>,
        headers: &mut [ContactHeader],
        contacts: &mut [ContactGeometry],
    ) -> &[u32] {
        let workers = executor.workers();
        if self.buckets.len() != workers {
            self.buckets = PerWorker::new(workers, |_| Default::default());
            self.changed = PerWorker::new(workers, |_| BitSet::default());
            self.scratch = PerWorker::new(workers, |_| TriangulatedScratch::new(input.tolerance));
        }
        self.pools.begin(workers);
        let stamp = self.pools.stamp();
        let mut previous = std::mem::take(&mut self.previous);
        let current = self.pools.split(&mut previous);
        for changed in self.changed.iter_mut() {
            changed.clear_all();
            changed.grow(headers.len());
        }
        let ctx = KernelContext {
            previous: &previous,
            current,
            scratch: &self.scratch,
            stamp,
            headers: Shared::new(headers),
            contacts: Shared::new(contacts),
            colliders: input.colliders,
            frames: input.frames,
            speculative: input.tolerance.speculative_distance(),
        };
        let recycle = 10.0 * input.tolerance.linear_slop();
        let speculative = input.tolerance.speculative_distance();
        let awake = input.awake;
        let buckets = &self.buckets;
        let changed = &self.changed;
        let classify_width =
            (awake.len() / (workers * BLOCKS_PER_WORKER)).clamp(CLASSIFY_BLOCK.0, CLASSIFY_BLOCK.1);
        let classify = |block: Block| {
            let mut buckets = buckets.local(block.worker);
            let mut changed = changed.local(block.worker);
            let range = block.range(classify_width, awake.len());
            for index in range.clone() {
                if let Some(&far) = awake.get(index + 3 * PREFETCH_DISTANCE) {
                    ctx.headers.prefetch(far as usize, usize::MAX);
                }
                if index + PREFETCH_DISTANCE < range.end {
                    let near = awake[index + PREFETCH_DISTANCE] as usize;
                    let ahead = unsafe { ctx.headers.at(near) };
                    if matches!(ahead.kind, PairKind::CuboidCuboid | PairKind::General) {
                        ctx.contacts.prefetch(near, usize::MAX);
                    } else if ahead.points > 0 {
                        ctx.contacts
                            .prefetch(near, ContactGeometry::recycle_bytes(ahead.points));
                    }
                    prefetch(&input.fat[ahead.collider_a as usize], 64);
                    prefetch(&input.fat[ahead.collider_b as usize], 64);
                    prefetch(&input.frames[ahead.frame_a as usize], 64);
                    prefetch(&input.frames[ahead.frame_b as usize], 64);
                }
                let id = awake[index];
                let header = ctx.header(id);
                let fat_a = &input.fat[header.collider_a as usize];
                let fat_b = &input.fat[header.collider_b as usize];
                if !fat_a.overlaps(fat_b) {
                    if header.flags & TOUCHING != 0 {
                        header.flags |= STOPPED_TOUCHING;
                    }
                    header.flags = (header.flags & !TOUCHING) | DISJOINT;
                    changed.set(id as usize);
                    continue;
                }
                let (frame_a, frame_b) = ctx.frames(header);
                if input.recycle
                    && let Some(refresh) =
                        recycle::try_recycle(header, frame_a, frame_b, recycle, speculative)
                {
                    if header.points > 0 {
                        refresh.apply(&mut ctx.contact(id).manifold);
                    }
                    if header.kind == PairKind::Triangulated {
                        triangulated::carry_extras(&ctx, header, block.worker, &refresh);
                    }
                    continue;
                }
                match header.kind {
                    PairKind::CuboidCuboid | PairKind::General => {
                        general::collide_one(id, &ctx, &mut changed)
                    }
                    kind => buckets[kind.index()].push(id),
                }
            }
        };
        executor.run(
            &[Stage::for_items(
                "narrowphase contacts",
                awake.len(),
                classify_width,
            )],
            &classify,
        );

        for list in &mut self.lists {
            list.clear();
        }
        for buckets in self.buckets.iter_mut() {
            for (list, bucket) in self.lists.iter_mut().zip(buckets.iter_mut()) {
                list.append(bucket);
            }
        }
        let shares = self
            .lists
            .each_ref()
            .map(|list| worker_reserve(list.len(), workers));
        for buckets in self.buckets.iter_mut() {
            for (&share, bucket) in shares.iter().zip(buckets.iter_mut()) {
                bucket.reserve(share);
            }
        }
        let widths = PairKind::ALL.map(|kind| {
            let len = self.lists[kind.index()].len();
            match kind {
                PairKind::Triangulated => (len / (workers * BLOCKS_PER_WORKER))
                    .clamp(TRIANGULATED_BLOCK, MAX_TRIANGULATED_BLOCK),
                _ => simd.lanes() * KERNEL_CHUNKS,
            }
        });
        let mut firsts = [0u32; PairKind::COUNT + 1];
        for kind in PairKind::ALL {
            let blocks = self.lists[kind.index()]
                .len()
                .div_ceil(widths[kind.index()]);
            firsts[kind.index() + 1] = firsts[kind.index()] + blocks as u32;
        }
        let lists = &self.lists;
        let collide = |block: Block| {
            let slot = firsts.partition_point(|&first| first <= block.index) - 1;
            let kind = PairKind::ALL[slot];
            let list = &lists[slot];
            let local = Block {
                index: block.index - firsts[slot],
                ..block
            };
            let range = local.range(widths[slot], list.len());
            let window = &list[range.start..(range.end + PREFETCH_DISTANCE).min(list.len())];
            let mut changed = changed.local(block.worker);
            simd.run(CollideBlock {
                kind,
                window,
                count: range.len(),
                ctx: &ctx,
                changed: &mut changed,
                worker: block.worker,
            });
        };
        if firsts[PairKind::COUNT] > 0 {
            executor.run(
                &[Stage::new("narrowphase batches", firsts[PairKind::COUNT])],
                &collide,
            );
        }
        let count = contacts_len(&ctx);
        self.previous = previous;

        self.merged.clear_all();
        self.merged.grow(count);
        for changed in self.changed.iter_mut() {
            self.merged.union_with(changed);
        }
        self.changes.clear();
        self.changes
            .extend(self.merged.iter().map(|bit| bit as u32));
        &self.changes
    }
}

fn contacts_len(ctx: &KernelContext<'_>) -> usize {
    ctx.headers.len()
}
