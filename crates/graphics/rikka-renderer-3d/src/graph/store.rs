use std::mem::ManuallyDrop;
use std::sync::Arc;

use ash::vk;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::Result;
use crate::gpu::NONE_INDEX;
use crate::graph::builder::{FrameGraph, Source};
use crate::graph::compile::{self, Lifetime, Plan};
use crate::graph::heap::{self, Demand};
use crate::graph::resource::{Filter, ImageId, Import, Keep, is_depth};
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::{GpuImage, ImageDesc};
use rikka_gpu::core::memory::Allocation;
use rikka_gpu::core::runtime::Recycler;

const ATTACHMENTS: vk::ImageUsageFlags = vk::ImageUsageFlags::from_raw(
    vk::ImageUsageFlags::COLOR_ATTACHMENT.as_raw()
        | vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT.as_raw(),
);

const HEAP_NAME: &str = "rikka.three.graph_heap";

const GRAPH_TARGET: &str = "rikka_renderer::graph";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct PhysicalDesc {
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub usage: vk::ImageUsageFlags,
    pub filter: Filter,
    pub samples: vk::SampleCountFlags,
}

impl PhysicalDesc {
    fn image_desc(self) -> ImageDesc {
        match is_depth(self.format) {
            true => ImageDesc::depth(self.extent, self.format, self.usage),
            false => ImageDesc::color(self.extent, self.format, self.usage),
        }
        .samples(self.samples)
    }

    fn sampled_layout(self) -> vk::ImageLayout {
        match self.usage.contains(vk::ImageUsageFlags::STORAGE) {
            true => vk::ImageLayout::GENERAL,
            false => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }
    }

    fn holds(self, wanted: Self) -> bool {
        self.format == wanted.format
            && self.extent == wanted.extent
            && self.filter == wanted.filter
            && self.samples == wanted.samples
            && self.usage.contains(wanted.usage)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundImage {
    pub sampled: u32,
    pub storage: u32,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub extent: vk::Extent2D,
    pub physical: vk::Extent2D,
    pub layout: vk::ImageLayout,
    pub fresh: bool,
    pub alias: Option<MemoryAccess>,
}

impl BoundImage {
    const SINGLE: vk::Extent2D = vk::Extent2D {
        width: 1,
        height: 1,
    };

    const NONE: Self = Self {
        sampled: NONE_INDEX,
        storage: NONE_INDEX,
        view: vk::ImageView::null(),
        sampler: vk::Sampler::null(),
        extent: Self::SINGLE,
        physical: Self::SINGLE,
        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        fresh: false,
        alias: None,
    };

    pub(crate) fn imported(image: &GpuImage, sampled: u32) -> Self {
        Self {
            sampled,
            storage: NONE_INDEX,
            view: image.view(),
            sampler: vk::Sampler::null(),
            extent: image.extent(),
            physical: image.extent(),
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            fresh: false,
            alias: None,
        }
    }

    pub(crate) fn storing(mut self) -> Self {
        self.layout = vk::ImageLayout::GENERAL;
        self
    }

    pub(crate) fn with_sampler(mut self, sampler: vk::Sampler) -> Self {
        self.sampler = sampler;
        self
    }

    pub(crate) fn with_extent(mut self, extent: vk::Extent2D) -> Self {
        self.extent = extent;
        self
    }

    pub(crate) fn is_placed(&self) -> bool {
        self.extent != self.physical
    }
}

pub(crate) struct PhysicalImage {
    pub(super) image: GpuImage,
    desc: PhysicalDesc,
    sampler: vk::Sampler,
    sampled: u32,
    storage: u32,
    used: bool,
}

impl PhysicalImage {
    fn create(
        ctx: &mut GpuFrameCtx<'_>,
        name: &'static str,
        desc: PhysicalDesc,
        placed: Option<(vk::DeviceMemory, u64)>,
    ) -> Result<Self> {
        let image_desc = desc.image_desc();
        let image = match placed {
            Some((memory, offset)) => {
                GpuImage::placed(ctx.device.clone(), name, &image_desc, memory, offset)?
            }
            None => GpuImage::create(ctx.device.clone(), name, &image_desc)?,
        };
        let sampler = match (is_depth(desc.format), desc.filter) {
            (true, _) | (false, Filter::Nearest) => ctx.samplers.nearest_clamp(),
            (false, Filter::Linear) => ctx.samplers.linear_clamp(),
        };
        let sampled = match desc.usage.contains(vk::ImageUsageFlags::SAMPLED) {
            true => ctx
                .resources
                .allocate_in(image.view(), sampler, desc.sampled_layout())?,
            false => NONE_INDEX,
        };
        let storage = match desc.usage.contains(vk::ImageUsageFlags::STORAGE) {
            true => ctx.resources.allocate_storage(image.view())?,
            false => NONE_INDEX,
        };
        Ok(Self {
            image,
            desc,
            sampler,
            sampled,
            storage,
            used: false,
        })
    }

    fn bind(&mut self, extent: vk::Extent2D, alias: Option<MemoryAccess>) -> BoundImage {
        let fresh = !self.used;
        self.used = true;
        BoundImage {
            sampled: self.sampled,
            storage: self.storage,
            view: self.image.view(),
            sampler: self.sampler,
            extent,
            physical: self.desc.extent,
            layout: self.desc.sampled_layout(),
            fresh,
            alias,
        }
    }

    fn release(self, recycler: &mut Recycler<'_>) {
        let value = recycler.value;
        if self.sampled != NONE_INDEX {
            recycler.resources.release(self.sampled, value);
        }
        if self.storage != NONE_INDEX {
            recycler.resources.release_storage(self.storage, value);
        }
        recycler.retire.defer(value, Box::new(self.image));
    }
}

type StoreKey = (&'static str, u64);

#[derive(Default)]
pub(crate) struct ImageStore {
    entries: FxHashMap<StoreKey, PhysicalImage>,
    touched: FxHashSet<StoreKey>,
}

impl ImageStore {
    fn ensure(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        key: StoreKey,
        wanted: PhysicalDesc,
    ) -> Result<&mut PhysicalImage> {
        self.touched.insert(key);
        let reusable = self
            .entries
            .get(&key)
            .is_some_and(|entry| entry.desc.holds(wanted));
        if !reusable {
            let usage = self
                .entries
                .get(&key)
                .filter(|entry| {
                    entry.desc.extent == wanted.extent && entry.desc.format == wanted.format
                })
                .map_or(wanted.usage, |entry| entry.desc.usage | wanted.usage);
            let created =
                PhysicalImage::create(ctx, key.0, PhysicalDesc { usage, ..wanted }, None)?;
            if let Some(stale) = self.entries.insert(key, created) {
                stale.release(&mut ctx.recycler());
            }
        }
        Ok(self.entries.get_mut(&key).expect("entry was ensured"))
    }

    fn get_mut(&mut self, key: StoreKey) -> Option<&mut PhysicalImage> {
        self.entries.get_mut(&key)
    }

    pub(crate) fn holds_shared(&self, name: &'static str) -> bool {
        self.entries
            .get(&(name, 0))
            .is_some_and(|entry| entry.image.holds_content())
    }

    fn sweep(&mut self, recycler: &mut Recycler<'_>) {
        let touched = &self.touched;
        let stale: Vec<StoreKey> = self
            .entries
            .keys()
            .filter(|key| !touched.contains(*key))
            .copied()
            .collect();
        for key in stale {
            if let Some(entry) = self.entries.remove(&key) {
                entry.release(recycler);
            }
        }
        self.touched.clear();
    }

    pub(crate) fn release(self, recycler: &mut Recycler<'_>) {
        for (_, entry) in self.entries {
            entry.release(recycler);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PhysicalKey {
    Lane(PhysicalDesc, u32),
    Placed(PhysicalDesc, u64),
}

#[derive(Clone, Copy, Debug)]
enum Binding {
    Unused,
    Pool(usize),
    History(StoreKey),
    Shared(StoreKey),
    Imported(Import),
}

struct HeapBlock {
    device: Arc<RenderDevice>,
    allocation: ManuallyDrop<Allocation>,
    size: u64,
    types: u32,
}

impl HeapBlock {
    fn memory(&self) -> (vk::DeviceMemory, u64) {
        (self.allocation.memory(), self.allocation.offset())
    }

    fn serves(&self, size: u64, types: u32) -> bool {
        self.types == types && self.size >= size && self.size <= size.saturating_mul(2)
    }
}

impl Drop for HeapBlock {
    fn drop(&mut self) {
        let allocation = unsafe { ManuallyDrop::take(&mut self.allocation) };
        self.device.free(allocation);
    }
}

#[derive(Default)]
pub(crate) struct Imports {
    pub(crate) bound: [Option<BoundImage>; Import::COUNT],
}

#[derive(Default)]
pub(crate) struct ViewGraph {
    pub(super) plan: Option<Plan>,
    bindings: Vec<Binding>,
    pool: Vec<(PhysicalKey, PhysicalImage)>,
    heap: Option<HeapBlock>,
    history: ImageStore,
    pub(super) bound: Vec<BoundImage>,
    alias: Vec<Option<MemoryAccess>>,
    pub(super) barriers: Vec<vk::ImageMemoryBarrier2<'static>>,
    turn: u64,
}

impl ViewGraph {
    pub(crate) fn realize(
        &mut self,
        graph: &FrameGraph<'_>,
        ctx: &mut GpuFrameCtx<'_>,
        shared: &mut ImageStore,
        imports: &Imports,
    ) -> Result<()> {
        let signature = compile::signature(graph);
        if self
            .plan
            .as_ref()
            .is_none_or(|plan| plan.signature != signature)
        {
            let plan = compile::compile(graph, signature);
            tracing::debug!(
                target: GRAPH_TARGET,
                declared = graph.passes.len(),
                live = plan.steps.len(),
                images = graph.images.len(),
                "render graph compiled"
            );
            self.place(graph, &plan, ctx)?;
            self.plan = Some(plan);
        }
        self.turn = 1 - self.turn;
        self.bind(graph, ctx, shared, imports)
    }

    pub(crate) fn bound(&self, image: ImageId) -> BoundImage {
        self.bound[image.index()]
    }

    fn desc(
        graph: &FrameGraph<'_>,
        plan: &Plan,
        image: usize,
    ) -> Option<(&'static str, PhysicalDesc)> {
        match graph.images[image].source {
            Source::Created(spec) => Some((
                spec.name,
                PhysicalDesc {
                    format: spec.format,
                    extent: graph.physical_extent(spec),
                    usage: plan.usage[image],
                    filter: spec.filter,
                    samples: spec.samples,
                },
            )),
            Source::Imported(_) => None,
        }
    }

    fn place(
        &mut self,
        graph: &FrameGraph<'_>,
        plan: &Plan,
        ctx: &mut GpuFrameCtx<'_>,
    ) -> Result<()> {
        let count = graph.images.len();
        let mut bindings = vec![Binding::Unused; count];
        let mut wanted: Vec<(PhysicalKey, &'static str)> = Vec::new();
        let mut lanes: Vec<(PhysicalDesc, usize, u32)> = Vec::new();
        let mut placed: Vec<(usize, PhysicalDesc, &'static str, Demand, u32)> = Vec::new();

        let mut order: Vec<(usize, Lifetime)> = (0..count)
            .filter_map(|image| Some((image, plan.lifetimes[image]?)))
            .collect();
        order.sort_by_key(|(image, lifetime)| (lifetime.first, *image));
        for (image, lifetime) in order {
            let node = graph.images[image];
            let Source::Created(spec) = node.source else {
                continue;
            };
            if spec.keep != Keep::Transient {
                continue;
            }
            let (name, desc) = Self::desc(graph, plan, image).expect("created image has a desc");
            if !node.published && !desc.usage.intersects(ATTACHMENTS) {
                let requirements = GpuImage::requirements(ctx.device, &desc.image_desc());
                placed.push((
                    image,
                    desc,
                    name,
                    Demand {
                        size: requirements.size,
                        alignment: requirements.alignment,
                        lifetime,
                    },
                    requirements.memory_type_bits,
                ));
                continue;
            }
            let lane = match lanes
                .iter_mut()
                .find(|(known, busy, _)| *known == desc && *busy < lifetime.first)
            {
                Some(lane) => {
                    lane.1 = lifetime.last;
                    lane.2
                }
                None => {
                    let ordinal = lanes.iter().filter(|(known, ..)| *known == desc).count() as u32;
                    lanes.push((desc, lifetime.last, ordinal));
                    ordinal
                }
            };
            bindings[image] = Binding::Pool(wanted.len());
            wanted.push((PhysicalKey::Lane(desc, lane), name));
        }

        let types = placed.iter().fold(u32::MAX, |types, entry| types & entry.4);
        if types == 0 {
            for (image, desc, name, ..) in placed.drain(..) {
                bindings[image] = Binding::Pool(wanted.len());
                let ordinal = wanted.len() as u32;
                wanted.push((PhysicalKey::Lane(desc, u32::MAX - ordinal), name));
            }
        }
        let demands: Vec<Demand> = placed.iter().map(|entry| entry.3).collect();
        let placement = heap::place(&demands);
        tracing::debug!(
            target: GRAPH_TARGET,
            heap = placement.size,
            unaliased = demands.iter().map(|demand| demand.size).sum::<u64>(),
            placed = demands.len(),
            lanes = wanted.len(),
            "render graph images placed"
        );
        let mut renewed = false;
        if placed.is_empty() {
            renewed = self.heap.take().is_some();
        } else if !self
            .heap
            .as_ref()
            .is_some_and(|block| block.serves(placement.size, types))
        {
            let allocation = ctx.device.allocate_heap(
                HEAP_NAME,
                vk::MemoryRequirements {
                    size: placement.size,
                    alignment: placement.alignment,
                    memory_type_bits: types,
                },
            )?;
            let block = HeapBlock {
                device: ctx.device.clone(),
                allocation: ManuallyDrop::new(allocation),
                size: placement.size,
                types,
            };
            if let Some(old) = self.heap.replace(block) {
                ctx.retire.defer(ctx.frame_value, Box::new(old));
            }
            renewed = true;
        }
        for (at, (image, desc, name, ..)) in placed.iter().enumerate() {
            bindings[*image] = Binding::Pool(wanted.len());
            wanted.push((PhysicalKey::Placed(*desc, placement.offsets[at]), name));
        }

        let mut kept: FxHashMap<PhysicalKey, Vec<PhysicalImage>> = FxHashMap::default();
        let mut recycler = ctx.recycler();
        for (key, image) in self.pool.drain(..) {
            match (key, renewed) {
                (PhysicalKey::Placed(..), true) => image.release(&mut recycler),
                _ => kept.entry(key).or_default().push(image),
            }
        }
        let mut pool = Vec::with_capacity(wanted.len());
        for (key, name) in wanted {
            let mut image = match kept.get_mut(&key).and_then(Vec::pop) {
                Some(image) => image,
                None => {
                    let (desc, memory) = match key {
                        PhysicalKey::Lane(desc, _) => (desc, None),
                        PhysicalKey::Placed(desc, offset) => {
                            let (memory, base) = self
                                .heap
                                .as_ref()
                                .expect("placed images have a heap")
                                .memory();
                            (desc, Some((memory, base + offset)))
                        }
                    };
                    PhysicalImage::create(ctx, name, desc, memory)?
                }
            };
            if let PhysicalKey::Placed(..) = key {
                image.used = false;
            }
            pool.push((key, image));
        }
        let mut recycler = ctx.recycler();
        for image in kept.into_values().flatten() {
            image.release(&mut recycler);
        }

        let mut alias = vec![None; count];
        for (at, (image, ..)) in placed.iter().enumerate() {
            let range = (placement.offsets[at], placed[at].3.size);
            let after = placed
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != at)
                .filter(|&(other, entry)| {
                    heap::overlap(range, (placement.offsets[other], entry.3.size))
                })
                .fold(
                    MemoryAccess::new(vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
                    |union, (_, entry)| {
                        let touched = plan.touches[entry.0];
                        MemoryAccess::new(
                            union.stage | touched.stage,
                            union.access | touched.access,
                        )
                    },
                );
            alias[*image] = Some(after);
        }
        self.alias = alias;
        self.pool = pool;
        self.bindings = bindings;
        Ok(())
    }

    fn bind(
        &mut self,
        graph: &FrameGraph<'_>,
        ctx: &mut GpuFrameCtx<'_>,
        shared: &mut ImageStore,
        imports: &Imports,
    ) -> Result<()> {
        let plan = self
            .plan
            .as_ref()
            .expect("a plan is compiled before binding");
        self.bound.clear();
        for (image, node) in graph.images.iter().enumerate() {
            let extent = match node.source {
                Source::Created(spec) => graph.logical_extent(spec),
                Source::Imported(_) => vk::Extent2D::default(),
            };
            let binding = match (node.source, self.bindings[image]) {
                (Source::Imported(import), _) => Binding::Imported(import),
                (Source::Created(spec), Binding::Pool(index)) if spec.keep == Keep::Transient => {
                    Binding::Pool(index)
                }
                (Source::Created(spec), _) => match spec.keep {
                    Keep::Transient => Binding::Unused,
                    Keep::History(key) => Binding::History((spec.name, key)),
                    Keep::Current(key) => {
                        Binding::History((spec.name, key.wrapping_mul(2) + self.turn))
                    }
                    Keep::Previous(key) => {
                        Binding::History((spec.name, key.wrapping_mul(2) + (1 - self.turn)))
                    }
                    Keep::Shared => Binding::Shared((spec.name, 0)),
                },
            };
            let bound = match binding {
                Binding::Unused => BoundImage::NONE,
                Binding::Pool(index) => self.pool[index].1.bind(extent, self.alias[image]),
                Binding::History(key) | Binding::Shared(key) => {
                    let (_, desc) =
                        Self::desc(graph, plan, image).expect("created image has a desc");
                    let store = match binding {
                        Binding::History(_) => &mut self.history,
                        _ => &mut *shared,
                    };
                    store.ensure(ctx, key, desc)?.bind(extent, None)
                }
                Binding::Imported(import) => imports.bound[import.index()]
                    .expect("imported images are present in the frames that declare them"),
            };
            self.bindings[image] = binding;
            self.bound.push(bound);
        }
        self.history.sweep(&mut ctx.recycler());
        Ok(())
    }

    pub(super) fn physical_mut<'x>(
        &'x mut self,
        image: ImageId,
        shared: &'x mut ImageStore,
    ) -> Option<&'x mut GpuImage> {
        match self.bindings[image.index()] {
            Binding::Pool(index) => Some(&mut self.pool[index].1.image),
            Binding::History(key) => self.history.get_mut(key).map(|entry| &mut entry.image),
            Binding::Shared(key) => shared.get_mut(key).map(|entry| &mut entry.image),
            Binding::Unused | Binding::Imported(_) => None,
        }
    }

    pub(super) fn import_of(&self, image: ImageId) -> Option<Import> {
        match self.bindings[image.index()] {
            Binding::Imported(import) => Some(import),
            _ => None,
        }
    }

    pub(crate) fn release(self, recycler: &mut Recycler<'_>) {
        for (_, image) in self.pool {
            image.release(recycler);
        }
        self.history.release(recycler);
        if let Some(heap) = self.heap {
            recycler.retire.defer(recycler.value, Box::new(heap));
        }
    }
}
