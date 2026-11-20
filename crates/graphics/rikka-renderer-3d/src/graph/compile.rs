use std::hash::{Hash, Hasher};
use std::ops::Range;

use ash::vk;
use rustc_hash::FxHasher;

use crate::graph::builder::{FrameGraph, Source};
use crate::graph::resource::{Buffer, BufferUse, ImageId, ImageUse, Keep, SHADER_STAGES};
use rikka_gpu::core::barrier::MemoryAccess;

#[derive(Clone, Copy, Debug)]
pub(super) struct Transition {
    pub image: ImageId,
    pub usage: ImageUse,
    pub stage: vk::PipelineStageFlags2,
    pub discard: bool,
    pub first: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Step {
    pub pass: usize,
    pub before: Range<usize>,
    pub after: Range<usize>,
    pub memory: Option<(MemoryAccess, MemoryAccess)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Lifetime {
    pub first: usize,
    pub last: usize,
}

impl Lifetime {
    pub(super) fn overlaps(self, other: Self) -> bool {
        self.first <= other.last && other.first <= self.last
    }
}

pub(super) struct Plan {
    pub signature: u64,
    pub steps: Vec<Step>,
    pub transitions: Vec<Transition>,
    pub usage: Vec<vk::ImageUsageFlags>,
    pub lifetimes: Vec<Option<Lifetime>>,
    pub touches: Vec<MemoryAccess>,
}

pub(super) fn signature(graph: &FrameGraph<'_>) -> u64 {
    let mut hasher = FxHasher::default();
    graph.extents.capacity.width.hash(&mut hasher);
    graph.extents.capacity.height.hash(&mut hasher);
    graph.traced.hash(&mut hasher);
    for node in &graph.images {
        node.source.hash(&mut hasher);
        node.published.hash(&mut hasher);
    }
    for pass in &graph.passes {
        (pass.name.as_ptr() as usize, pass.name.len()).hash(&mut hasher);
        pass.group
            .map(|group| group.as_ptr() as usize)
            .hash(&mut hasher);
        graph.image_accesses[pass.images.clone()].hash(&mut hasher);
        graph.buffer_accesses[pass.buffers.clone()].hash(&mut hasher);
    }
    hasher.finish()
}

fn persists(graph: &FrameGraph<'_>, image: ImageId) -> bool {
    let node = graph.images[image.index()];
    node.published
        || match node.source {
            Source::Imported(_) => true,
            Source::Created(spec) => spec.keep != Keep::Transient,
        }
}

fn live_passes(graph: &FrameGraph<'_>) -> Vec<bool> {
    let count = graph.passes.len();
    let mut writer: Vec<Option<usize>> = vec![None; graph.images.len()];
    let mut buffer_writer: [Option<usize>; Buffer::COUNT] = [None; Buffer::COUNT];
    let mut depends: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut roots = vec![false; count];
    for (index, pass) in graph.passes.iter().enumerate() {
        for access in &graph.image_accesses[pass.images.clone()] {
            let slot = &mut writer[access.image.index()];
            if !access.discard
                && let Some(source) = *slot
            {
                depends[index].push(source);
            }
            if access.usage.writes() {
                *slot = Some(index);
                roots[index] |= persists(graph, access.image);
            }
        }
        for access in &graph.buffer_accesses[pass.buffers.clone()] {
            let slot = &mut buffer_writer[access.buffer.index()];
            if let Some(source) = *slot {
                depends[index].push(source);
            }
            if access.usage.writes() {
                *slot = Some(index);
                roots[index] = true;
            }
        }
    }
    let mut live = roots;
    for index in (0..count).rev() {
        if !live[index] {
            continue;
        }
        for &source in &depends[index] {
            debug_assert!(source < index, "passes depend only on earlier passes");
            live[source] = true;
        }
    }
    live
}

#[derive(Clone, Copy)]
struct BufferState {
    write: Option<BufferUse>,
    readers: BufferUse,
    visible: BufferUse,
}

fn union(a: MemoryAccess, b: MemoryAccess) -> MemoryAccess {
    MemoryAccess::new(a.stage | b.stage, a.access | b.access)
}

fn as_memory(usage: BufferUse) -> MemoryAccess {
    MemoryAccess::new(usage.stage, usage.access)
}

pub(super) fn compile(graph: &FrameGraph<'_>, signature: u64) -> Plan {
    let live = live_passes(graph);
    let image_count = graph.images.len();
    let mut usage: Vec<vk::ImageUsageFlags> = graph
        .images
        .iter()
        .map(|node| match node.source {
            Source::Created(spec) if !spec.sampleable() => vk::ImageUsageFlags::empty(),
            _ => vk::ImageUsageFlags::SAMPLED,
        })
        .collect();
    let mut lifetimes: Vec<Option<Lifetime>> = vec![None; image_count];
    let mut touches =
        vec![MemoryAccess::new(vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE); image_count];
    let mut last_write = vec![None; image_count];
    let mut seen = vec![false; image_count];
    let mut buffers: [BufferState; Buffer::COUNT] = std::array::from_fn(|_| BufferState {
        write: None,
        readers: BufferUse::NONE,
        visible: BufferUse::NONE,
    });
    for buffer in [
        Buffer::Deform,
        Buffer::Draws,
        Buffer::Clusters,
        Buffer::Environment,
        Buffer::History,
        Buffer::Scene,
        Buffer::Particles,
    ] {
        let carried = buffer.carried(graph.traced);
        buffers[buffer.index()] = BufferState {
            write: carried.write,
            readers: BufferUse::new(carried.readers, vk::AccessFlags2::NONE),
            visible: BufferUse::NONE,
        };
    }

    let mut steps = Vec::new();
    let mut transitions = Vec::new();
    for (index, pass) in graph.passes.iter().enumerate() {
        if !live[index] {
            continue;
        }
        let at = steps.len();
        let before = transitions.len();
        for access in &graph.image_accesses[pass.images.clone()] {
            let image = access.image.index();
            if let Source::Created(spec) = graph.images[image].source {
                usage[image] |= access.usage.usage(spec.format);
                assert!(
                    seen[image] || access.discard || spec.keep != Keep::Transient,
                    "pass {} reads {} before anything wrote it",
                    pass.name,
                    spec.name
                );
            }
            let first = !seen[image];
            seen[image] = true;
            let span = lifetimes[image].get_or_insert(Lifetime {
                first: at,
                last: at,
            });
            span.last = at;
            let touched = access.usage.access(access.stage, vk::ImageLayout::GENERAL);
            touches[image] = union(
                touches[image],
                MemoryAccess::new(touched.stage, touched.access),
            );
            if access.usage.writes() {
                last_write[image] = Some(at);
            }
            transitions.push(Transition {
                image: access.image,
                usage: access.usage,
                stage: access.stage,
                discard: access.discard,
                first,
            });
        }
        let mut memory: Option<(MemoryAccess, MemoryAccess)> = None;
        let mut barrier = |from: MemoryAccess, to: MemoryAccess| {
            memory = Some(match memory {
                Some((src, dst)) => (union(src, from), union(dst, to)),
                None => (from, to),
            });
        };
        for access in &graph.buffer_accesses[pass.buffers.clone()] {
            let state = &mut buffers[access.buffer.index()];
            let usage = access.usage;
            if usage.writes() {
                let mut from = MemoryAccess::new(state.readers.stage, vk::AccessFlags2::NONE);
                if let Some(write) = state.write {
                    from = union(from, as_memory(write));
                }
                if from.stage != vk::PipelineStageFlags2::NONE {
                    barrier(from, as_memory(usage));
                }
                *state = BufferState {
                    write: Some(usage),
                    readers: BufferUse::NONE,
                    visible: BufferUse::NONE,
                };
            } else {
                let covered = state.visible.stage.contains(usage.stage)
                    && state.visible.access.contains(usage.access);
                if let Some(write) = state.write
                    && !covered
                {
                    barrier(as_memory(write), as_memory(usage));
                    state.visible = BufferUse::new(
                        state.visible.stage | usage.stage,
                        state.visible.access | usage.access,
                    );
                }
                state.readers = BufferUse::new(
                    state.readers.stage | usage.stage,
                    state.readers.access | usage.access,
                );
            }
        }
        steps.push(Step {
            pass: index,
            before: before..transitions.len(),
            after: transitions.len()..transitions.len(),
            memory,
        });
    }

    let end = steps.len().saturating_sub(1);
    for (image, node) in graph.images.iter().enumerate() {
        if !node.published {
            continue;
        }
        if let Some(span) = &mut lifetimes[image] {
            span.first = 0;
            span.last = end;
        }
    }
    for (step_at, step) in steps.iter_mut().enumerate() {
        let start = transitions.len();
        for (image, written) in last_write.iter().enumerate() {
            if *written == Some(step_at) && graph.images[image].published {
                transitions.push(Transition {
                    image: ImageId(image as u32),
                    usage: ImageUse::Sampled,
                    stage: SHADER_STAGES,
                    discard: false,
                    first: false,
                });
            }
        }
        step.after = start..transitions.len();
    }
    merge_read_stages(&steps, &mut transitions, image_count);

    Plan {
        signature,
        steps,
        transitions,
        usage,
        lifetimes,
        touches,
    }
}

fn merge_read_stages(steps: &[Step], transitions: &mut [Transition], image_count: usize) {
    let mut runs: Vec<Vec<usize>> = vec![Vec::new(); image_count];
    let settle = |run: &mut Vec<usize>, transitions: &mut [Transition]| {
        let stage = run
            .iter()
            .fold(vk::PipelineStageFlags2::NONE, |stage, &at| {
                stage | transitions[at].stage
            });
        for &at in run.iter() {
            transitions[at].stage = stage;
        }
        run.clear();
    };
    let order = steps
        .iter()
        .flat_map(|step| step.before.clone().chain(step.after.clone()));
    for at in order {
        let transition = transitions[at];
        let run = &mut runs[transition.image.index()];
        match transition.usage {
            ImageUse::Sampled => run.push(at),
            _ => settle(run, transitions),
        }
    }
    for run in &mut runs {
        settle(run, transitions);
    }
}
