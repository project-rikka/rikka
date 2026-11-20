use rikka_gpu::Ticket;

use crate::fluid::Fluid;
use crate::stage::Frame;

#[must_use = "recorded surface is visible only after its batch is submitted and completed"]
pub struct Recorded {
    pub(crate) frame: Frame,
}

impl Fluid {
    pub fn commit(&mut self, recorded: Recorded, ready: &Ticket) {
        self.stage.present(recorded.frame, ready);
    }
}

impl std::fmt::Debug for Recorded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorded")
            .field("particles", &self.frame.particle_count)
            .field("spray", &self.frame.spray_count)
            .finish_non_exhaustive()
    }
}
