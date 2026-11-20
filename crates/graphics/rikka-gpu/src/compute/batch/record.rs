use ash::vk;
use rustc_hash::FxHashSet;

use crate::compute::context::Compute;
use crate::compute::timing::Clock;
use crate::error::Result;

use super::command::{Command, Key};
use super::dispatch::Launch;
use super::submit::BatchBarriers;

pub(super) fn record(
    compute: &Compute,
    cmd: vk::CommandBuffer,
    commands: &[Command],
    barriers: BatchBarriers<'_>,
    reads_back: bool,
    timed: Option<(&Clock, Vec<&'static str>)>,
) -> Result<()> {
    let shared = &compute.shared;
    let device = shared.device.handle();
    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe { device.begin_command_buffer(cmd, &begin)? };
    let stamps = match timed {
        Some((clock, scopes)) => {
            let count = scopes.len() as u32;
            clock.prepare(cmd, scopes)?.map(|pool| (pool, count))
        }
        None => None,
    };
    open_batch(device, cmd, &barriers);

    let sets = [shared.table.set()];
    let layout = shared.table.pipeline_layout();
    unsafe {
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, layout, 0, &sets, &[]);
    }
    let mut recorder = CommandRecorder {
        device,
        cmd,
        layout,
        bound: vk::Pipeline::null(),
        timestamps: stamps.map(|(pool, _)| pool),
    };
    let mut hazards = Hazards::default();
    for command in commands {
        let (reads, writes) = command.touches();
        if hazards.conflicts(reads, writes) {
            full_barrier(device, cmd);
            hazards.clear();
        }
        hazards.note(reads, writes);
        recorder.record(command);
    }

    if let Some((pool, count)) = stamps {
        unsafe {
            device.cmd_write_timestamp2(cmd, vk::PipelineStageFlags2::ALL_COMMANDS, pool, count)
        };
    }
    close_batch(device, cmd, &barriers, reads_back);
    unsafe { device.end_command_buffer(cmd)? };
    Ok(())
}

fn open_batch(device: &ash::Device, cmd: vk::CommandBuffer, barriers: &BatchBarriers<'_>) {
    let ahead = [vk::MemoryBarrier2::default()
        .src_stage_mask(WORK_STAGES)
        .src_access_mask(WRITES)
        .dst_stage_mask(WORK_STAGES)
        .dst_access_mask(WORK_ACCESS)];
    let layouts: Vec<vk::ImageMemoryBarrier2<'_>> = barriers
        .fresh
        .iter()
        .map(|image| {
            vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::NONE)
                .dst_stage_mask(WORK_STAGES)
                .dst_access_mask(WORK_ACCESS)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::GENERAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image.handle())
                .subresource_range(image.range())
        })
        .chain(barriers.opening.iter().copied())
        .collect();
    let dependency = vk::DependencyInfo::default()
        .memory_barriers(&ahead)
        .image_memory_barriers(&layouts);
    unsafe { device.cmd_pipeline_barrier2(cmd, &dependency) };
}

fn close_batch(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    barriers: &BatchBarriers<'_>,
    reads_back: bool,
) {
    if !barriers.closing.is_empty() {
        let dependency = vk::DependencyInfo::default().image_memory_barriers(barriers.closing);
        unsafe { device.cmd_pipeline_barrier2(cmd, &dependency) };
    }
    if reads_back {
        let after = [vk::MemoryBarrier2::default()
            .src_stage_mask(WORK_STAGES)
            .src_access_mask(WRITES)
            .dst_stage_mask(vk::PipelineStageFlags2::HOST)
            .dst_access_mask(vk::AccessFlags2::HOST_READ)];
        let dependency = vk::DependencyInfo::default().memory_barriers(&after);
        unsafe { device.cmd_pipeline_barrier2(cmd, &dependency) };
    }
}

struct CommandRecorder<'a> {
    device: &'a ash::Device,
    cmd: vk::CommandBuffer,
    layout: vk::PipelineLayout,
    bound: vk::Pipeline,
    timestamps: Option<vk::QueryPool>,
}

impl CommandRecorder<'_> {
    fn record(&mut self, command: &Command) {
        let (device, cmd) = (self.device, self.cmd);
        unsafe {
            match command {
                Command::Dispatch {
                    pipeline,
                    launch,
                    push,
                    ..
                } => self.dispatch(*pipeline, *launch, push.bytes()),
                Command::Upload {
                    image,
                    staging,
                    region,
                    ..
                } => device.cmd_copy_buffer_to_image(
                    cmd,
                    *staging,
                    *image,
                    vk::ImageLayout::GENERAL,
                    std::slice::from_ref(region),
                ),
                Command::Copy {
                    from, to, region, ..
                } => device.cmd_copy_image(
                    cmd,
                    *from,
                    vk::ImageLayout::GENERAL,
                    *to,
                    vk::ImageLayout::GENERAL,
                    std::slice::from_ref(region),
                ),
                Command::Clear {
                    image,
                    range,
                    color,
                    ..
                } => device.cmd_clear_color_image(
                    cmd,
                    *image,
                    vk::ImageLayout::GENERAL,
                    &vk::ClearColorValue { float32: *color },
                    std::slice::from_ref(range),
                ),
                Command::Fill {
                    buffer,
                    at,
                    size,
                    value,
                    ..
                } => device.cmd_fill_buffer(cmd, *buffer, *at, *size, *value),
                Command::WriteBuffer {
                    staging,
                    buffer,
                    region,
                    ..
                } => device.cmd_copy_buffer(cmd, *staging, *buffer, std::slice::from_ref(region)),
                Command::ImageOut {
                    image,
                    download_slot,
                    region,
                    ..
                } => device.cmd_copy_image_to_buffer(
                    cmd,
                    *image,
                    vk::ImageLayout::GENERAL,
                    *download_slot,
                    std::slice::from_ref(region),
                ),
                Command::BufferOut {
                    buffer,
                    download_slot,
                    region,
                    ..
                } => device.cmd_copy_buffer(
                    cmd,
                    *buffer,
                    *download_slot,
                    std::slice::from_ref(region),
                ),
                Command::CopyBuffer {
                    from, to, region, ..
                } => device.cmd_copy_buffer(cmd, *from, *to, std::slice::from_ref(region)),
                Command::Stamp { index } => {
                    if let Some(pool) = self.timestamps {
                        device.cmd_write_timestamp2(
                            cmd,
                            vk::PipelineStageFlags2::ALL_COMMANDS,
                            pool,
                            *index,
                        );
                    }
                }
            }
        }
    }

    fn dispatch(&mut self, pipeline: vk::Pipeline, launch: Launch, push: &[u8]) {
        let (device, cmd) = (self.device, self.cmd);
        unsafe {
            if pipeline != self.bound {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
                self.bound = pipeline;
            }
            if !push.is_empty() {
                device.cmd_push_constants(cmd, self.layout, vk::ShaderStageFlags::COMPUTE, 0, push);
            }
            match launch {
                Launch::Direct(groups) => device.cmd_dispatch(cmd, groups[0], groups[1], groups[2]),
                Launch::Indirect(counts, offset) => {
                    device.cmd_dispatch_indirect(cmd, counts, offset);
                }
            }
        }
    }
}

pub(super) const WORK_STAGES: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::from_raw(
    vk::PipelineStageFlags2::COMPUTE_SHADER.as_raw()
        | vk::PipelineStageFlags2::ALL_TRANSFER.as_raw()
        | vk::PipelineStageFlags2::DRAW_INDIRECT.as_raw(),
);

const WRITES: vk::AccessFlags2 = vk::AccessFlags2::from_raw(
    vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw() | vk::AccessFlags2::TRANSFER_WRITE.as_raw(),
);

const WORK_ACCESS: vk::AccessFlags2 = vk::AccessFlags2::from_raw(
    vk::AccessFlags2::SHADER_SAMPLED_READ.as_raw()
        | vk::AccessFlags2::SHADER_STORAGE_READ.as_raw()
        | vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw()
        | vk::AccessFlags2::TRANSFER_READ.as_raw()
        | vk::AccessFlags2::TRANSFER_WRITE.as_raw()
        | vk::AccessFlags2::INDIRECT_COMMAND_READ.as_raw(),
);

fn full_barrier(device: &ash::Device, cmd: vk::CommandBuffer) {
    let barrier = [vk::MemoryBarrier2::default()
        .src_stage_mask(WORK_STAGES)
        .src_access_mask(WRITES)
        .dst_stage_mask(WORK_STAGES)
        .dst_access_mask(WORK_ACCESS)];
    let dependency = vk::DependencyInfo::default().memory_barriers(&barrier);
    unsafe { device.cmd_pipeline_barrier2(cmd, &dependency) };
}

#[derive(Default)]
struct Hazards {
    written: FxHashSet<Key>,
    read: FxHashSet<Key>,
}

impl Hazards {
    fn conflicts(&self, reads: &[Key], writes: &[Key]) -> bool {
        reads.iter().any(|key| self.written.contains(key))
            || writes
                .iter()
                .any(|key| self.written.contains(key) || self.read.contains(key))
    }

    fn note(&mut self, reads: &[Key], writes: &[Key]) {
        self.read.extend(reads.iter().copied());
        self.written.extend(writes.iter().copied());
    }

    fn clear(&mut self) {
        self.written.clear();
        self.read.clear();
    }
}
