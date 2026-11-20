use std::sync::Arc;

use ash::vk;

use crate::compute::buffer::ComputeBuffer;
use crate::compute::context::Shared;
use crate::compute::foreign::{ForeignAcquire, ForeignImage};
use crate::compute::image::ComputeImage;
use crate::compute::kernel::Kernel;
use crate::compute::lane::{Lane, at};
use crate::compute::pool::Use;
use crate::compute::ticket::Ticket;
use crate::core::device::QueueRole;
use crate::error::Result;

use super::Batch;
use super::record::{WORK_STAGES, record};

pub(super) struct BatchResources<'a> {
    pub(super) images: &'a [ComputeImage],
    pub(super) buffers: &'a [ComputeBuffer],
    pub(super) kernels: &'a [Kernel],
    pub(super) foreign: &'a [ForeignImage],
}

pub(super) struct BatchBarriers<'a> {
    pub(super) fresh: &'a [&'a ComputeImage],
    pub(super) opening: &'a [vk::ImageMemoryBarrier2<'static>],
    pub(super) closing: &'a [vk::ImageMemoryBarrier2<'static>],
}

pub(super) fn send(
    shared: &Shared,
    lane: Lane,
    cmd: vk::CommandBuffer,
    resources: BatchResources<'_>,
    opening: &ForeignAcquire,
) -> Result<u64> {
    let BatchResources {
        images,
        buffers,
        kernels,
        foreign,
    } = resources;
    let mut submitted = shared.submitted.lock();
    let value = submitted[lane.index()] + 1;
    let other = lane.other();
    let needed = images
        .iter()
        .map(|image| image.uses()[other.index()])
        .chain(buffers.iter().map(|buffer| buffer.uses()[other.index()]))
        .max()
        .unwrap_or(0);
    let line = shared.line(other);
    let waits: Vec<vk::SemaphoreSubmitInfo<'_>> = (needed > line.timeline.completed())
        .then(|| {
            vk::SemaphoreSubmitInfo::default()
                .semaphore(line.timeline.handle())
                .value(needed)
                .stage_mask(WORK_STAGES)
        })
        .into_iter()
        .collect();
    let waits: Vec<vk::SemaphoreSubmitInfo<'_>> = waits
        .into_iter()
        .chain(opening.waits.iter().copied())
        .collect();
    let signals: Vec<vk::SemaphoreSubmitInfo<'_>> = std::iter::once(
        vk::SemaphoreSubmitInfo::default()
            .semaphore(shared.line(lane).timeline.handle())
            .value(value)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS),
    )
    .chain(opening.signals.iter().copied())
    .collect();
    let command_buffers = [vk::CommandBufferSubmitInfo::default().command_buffer(cmd)];
    let submit = vk::SubmitInfo2::default()
        .wait_semaphore_infos(&waits)
        .command_buffer_infos(&command_buffers)
        .signal_semaphore_infos(&signals);
    let role = match lane {
        Lane::Front => QueueRole::Graphics,
        Lane::Back => QueueRole::Background,
    };
    shared.device.submit(role, &[submit], vk::Fence::null())?;
    submitted[lane.index()] = value;
    for image in images {
        image.used_until(lane, value);
    }
    for buffer in buffers {
        buffer.used_until(lane, value);
    }
    for image in foreign {
        image.used_until(lane, value);
    }
    for kernel in kernels {
        kernel.used_until(lane, value);
    }
    Ok(value)
}

impl Batch {
    pub fn submit(self) -> Result<Ticket> {
        let Batch {
            compute,
            commands,
            images,
            buffers,
            foreign,
            bound_keys: _,
            kernels,
            staging,
            landings,
            scopes,
            clock,
            failed,
        } = self;
        if let Some(error) = failed {
            return Err(error);
        }
        let shared = &compute.shared;
        let lane = compute.lane;
        shared.collect();
        let device = &shared.device;
        let mut recorder = shared.recorder(lane)?;
        let cmd = match recorder.command_buffer(device, shared.line(lane).timeline.finished()?) {
            Ok(cmd) => cmd,
            Err(error) => {
                shared.recycle(lane, recorder);
                return Err(error);
            }
        };
        let family = shared.line(lane).family;
        let mut opening = ForeignAcquire::default();
        for image in &foreign {
            if let Err(error) = image.open(family, &mut opening) {
                recorder.recycle(cmd);
                shared.recycle(lane, recorder);
                return Err(error);
            }
        }
        let closing: Vec<vk::ImageMemoryBarrier2<'static>> = foreign
            .iter()
            .filter_map(|image| image.close(family))
            .collect();
        let fresh: Vec<&ComputeImage> = images
            .iter()
            .filter(|image| image.take_first_use())
            .collect();
        let timed = clock.as_ref().map(|clock| (clock.as_ref(), scopes));
        let recorded = record(
            &compute,
            cmd,
            &commands,
            BatchBarriers {
                fresh: &fresh,
                opening: &opening.barriers,
                closing: &closing,
            },
            !landings.is_empty(),
            timed,
        );
        let sent = recorded.and_then(|()| {
            send(
                shared,
                lane,
                cmd,
                BatchResources {
                    images: &images,
                    buffers: &buffers,
                    kernels: &kernels,
                    foreign: &foreign,
                },
                &opening,
            )
        });
        let value = match sent {
            Ok(value) => value,
            Err(error) => {
                recorder.recycle(cmd);
                shared.recycle(lane, recorder);
                for image in fresh {
                    image.forget_first_use();
                }
                return Err(error);
            }
        };
        recorder.submitted(value, cmd);
        shared.recycle(lane, recorder);
        for download_slot in &landings {
            download_slot.complete_at(value);
        }
        let mut state = shared.state.lock();
        for buffer in staging {
            let evicted = state
                .pool
                .recycle_buffer(Use::Staging, buffer, at(lane, value));
            state.release_eviction(evicted);
        }
        drop(state);
        drop(landings);
        if let Some(clock) = clock {
            clock.complete_at(value);
            shared.retire(at(lane, value), Box::new(clock));
        }
        drop((images, buffers, kernels, foreign));
        Ok(Ticket {
            shared: Arc::clone(shared),
            lane,
            value,
        })
    }
}
