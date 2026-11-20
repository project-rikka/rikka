use rikka_gpu::Ticket;
use rikka_physics_cloth as cloth;
use rikka_physics_fluid as fluid;

use crate::error::Result;

pub(crate) struct Recorded {
    pub(crate) fluid: fluid::Recorded,
    pub(crate) cloth: Option<cloth::Recorded>,
}

pub(crate) struct PendingCapture {
    pub(crate) fluid: fluid::Capture,
    pub(crate) cloth: Option<cloth::Capture>,
}

pub(crate) struct Capture {
    pub(crate) fluid: fluid::Capture,
    pub(crate) cloth: Option<cloth::Capture>,
    ready: Ticket,
}

pub(crate) struct PendingCheckpoint<'a> {
    pub(crate) fluid: fluid::PendingCheckpoint<'a>,
    pub(crate) cloth: Option<cloth::PendingCheckpoint<'a>>,
}

pub(crate) struct Checkpoint {
    pub(crate) fluid: fluid::Checkpoint,
    pub(crate) cloth: Option<cloth::Checkpoint>,
}

impl PendingCapture {
    pub(crate) fn submitted(self, ready: &Ticket) -> Capture {
        Capture {
            fluid: self.fluid,
            cloth: self.cloth,
            ready: ready.clone(),
        }
    }
}

impl Capture {
    pub(crate) fn bytes(&self) -> usize {
        self.fluid.bytes() + self.cloth.as_ref().map_or(0, cloth::Capture::bytes)
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.is_ready()
    }
}

impl PendingCheckpoint<'_> {
    pub(crate) fn finish(self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            fluid: self.fluid.finish()?,
            cloth: self
                .cloth
                .map(cloth::PendingCheckpoint::finish)
                .transpose()?,
        })
    }
}

impl Checkpoint {
    pub(crate) fn bytes(&self) -> usize {
        self.fluid.bytes() + self.cloth.as_ref().map_or(0, cloth::Checkpoint::bytes)
    }
}
