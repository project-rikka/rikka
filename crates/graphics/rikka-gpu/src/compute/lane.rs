use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::device::RenderDevice;
use crate::core::frame::FrameTimeline;
use crate::core::queue_lock::LockedQueue;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    Front,
    Back,
}

impl Lane {
    pub(crate) const ALL: [Lane; 2] = [Lane::Front, Lane::Back];

    pub(crate) fn index(self) -> usize {
        match self {
            Lane::Front => 0,
            Lane::Back => 1,
        }
    }

    pub(crate) fn other(self) -> Lane {
        match self {
            Lane::Front => Lane::Back,
            Lane::Back => Lane::Front,
        }
    }
}

pub(crate) type Marks = [u64; 2];

pub(crate) fn reached(marks: Marks, finished: Marks) -> bool {
    marks.iter().zip(finished).all(|(mark, done)| *mark <= done)
}

pub(crate) fn at(lane: Lane, value: u64) -> Marks {
    let mut marks = [0; 2];
    marks[lane.index()] = value;
    marks
}

#[derive(Debug, Default)]
pub(crate) struct Uses([AtomicU64; 2]);

impl Uses {
    pub(crate) fn mark(&self, lane: Lane, value: u64) {
        self.0[lane.index()].fetch_max(value, Ordering::AcqRel);
    }

    pub(crate) fn marks(&self) -> Marks {
        [
            self.0[0].load(Ordering::Acquire),
            self.0[1].load(Ordering::Acquire),
        ]
    }
}

pub(crate) struct Line {
    pub(crate) timeline: Arc<FrameTimeline>,
    pub(crate) family: u32,
}

impl Line {
    pub(crate) fn new(device: &Arc<RenderDevice>, lane: Lane) -> Result<Line> {
        Ok(Line {
            timeline: FrameTimeline::new(device.clone())?,
            family: match lane {
                Lane::Front => device.queue_family(),
                Lane::Back => device.background_queue().1,
            },
        })
    }

    pub(crate) fn queue(device: &RenderDevice, lane: Lane) -> &LockedQueue {
        match lane {
            Lane::Front => device.graphics_queue(),
            Lane::Back => device.background_queue().0,
        }
    }
}
