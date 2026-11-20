use ash::vk;

use crate::core::instance::VulkanInstance;
use crate::core::queue_lock::LockedQueue;
use crate::core::video_support::VideoSupport;

use super::request::DeviceRequest;
use super::selection::{Candidate, global_priority};

const BACKGROUND_PRIORITY: f32 = 0.5;

const GRAPHICS_WITH_BACKGROUND: [f32; 2] = [1.0, BACKGROUND_PRIORITY];

const SINGLE_PRIORITY: [f32; 1] = [1.0];

const GRAPHICS_QUEUES: u32 = 1;

pub(super) struct QueuePlan {
    background: Option<BackgroundPlacement>,
    pub(super) transfer: Option<u32>,
}

pub(super) struct Queues {
    pub(super) graphics: LockedQueue,
    pub(super) background: Option<FamilyQueue>,
    pub(super) transfer: Option<FamilyQueue>,
}

impl QueuePlan {
    pub(super) fn new(
        instance: &VulkanInstance,
        candidate: &Candidate,
        request: DeviceRequest,
        video: Option<&VideoSupport>,
    ) -> QueuePlan {
        let taken: Vec<u32> = video
            .map(|support| support.families.iter().map(|family| family.index).collect())
            .unwrap_or_default();
        let graphics_raised =
            request.prioritized && global_priority(&candidate.extensions).is_some();
        let background = (request.compute && request.background)
            .then(|| BackgroundPlacement::choose(instance, candidate, &taken))
            .flatten()
            .filter(|placement| {
                !graphics_raised || matches!(placement, BackgroundPlacement::Own(_))
            });
        let transfer = request
            .transfer
            .then(|| transfer_family(instance, candidate.physical, &taken))
            .flatten();
        QueuePlan {
            background,
            transfer,
        }
    }

    pub(super) fn queue_infos(
        &self,
        graphics_family: u32,
        video: Option<&VideoSupport>,
    ) -> Vec<vk::DeviceQueueCreateInfo<'static>> {
        let graphics_priorities: &'static [f32] = match self.background {
            Some(BackgroundPlacement::Beside(_)) => &GRAPHICS_WITH_BACKGROUND,
            _ => &SINGLE_PRIORITY,
        };
        let single = |family: u32| {
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&SINGLE_PRIORITY)
        };
        let mut infos = vec![
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(graphics_family)
                .queue_priorities(graphics_priorities),
        ];
        if let Some(BackgroundPlacement::Own(family)) = self.background {
            infos.push(single(family));
        }
        if let Some(family) = self.transfer {
            infos.push(single(family));
        }
        if let Some(support) = video {
            infos.extend(support.families.iter().map(|family| single(family.index)));
        }
        infos
    }

    pub(super) fn take_queues(&self, device: &ash::Device, graphics_family: u32) -> Queues {
        let queue = |family: u32, index: u32| {
            LockedQueue::new(unsafe { device.get_device_queue(family, index) })
        };
        let background = self.background.map(|placement| {
            let (family, index) = match placement {
                BackgroundPlacement::Own(family) => (family, 0),
                BackgroundPlacement::Beside(index) => (graphics_family, index),
            };
            FamilyQueue {
                family,
                queue: queue(family, index),
            }
        });
        let transfer = self.transfer.map(|family| FamilyQueue {
            family,
            queue: queue(family, 0),
        });
        if let Some(transfer) = &transfer {
            tracing::info!(
                family = transfer.family,
                "using dedicated transfer queue for uploads"
            );
        }
        Queues {
            graphics: queue(graphics_family, 0),
            background,
            transfer,
        }
    }
}

pub(super) struct FamilyQueue {
    pub(super) family: u32,
    pub(super) queue: LockedQueue,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BackgroundPlacement {
    Own(u32),
    Beside(u32),
}

impl BackgroundPlacement {
    pub(super) fn choose(
        instance: &VulkanInstance,
        candidate: &Candidate,
        taken: &[u32],
    ) -> Option<BackgroundPlacement> {
        let families = unsafe {
            instance
                .handle()
                .get_physical_device_queue_family_properties(candidate.physical)
        };
        let own = families.iter().zip(0u32..).find(|(family, index)| {
            family.queue_flags.contains(vk::QueueFlags::COMPUTE)
                && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && family.queue_count > 0
                && !taken.contains(index)
        });
        if let Some((_, index)) = own {
            return Some(BackgroundPlacement::Own(index));
        }
        (candidate.queue_count > GRAPHICS_QUEUES)
            .then_some(BackgroundPlacement::Beside(GRAPHICS_QUEUES))
    }
}

pub(super) fn transfer_family(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    taken: &[u32],
) -> Option<u32> {
    let families = unsafe {
        instance
            .handle()
            .get_physical_device_queue_family_properties(physical)
    };
    let single_texels = vk::Extent3D {
        width: 1,
        height: 1,
        depth: 1,
    };
    families
        .iter()
        .zip(0u32..)
        .find(|(family, index)| {
            family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !family
                    .queue_flags
                    .intersects(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                && family.queue_count > 0
                && family.min_image_transfer_granularity == single_texels
                && !taken.contains(index)
        })
        .map(|(_, index)| index)
}
