use crate::cff::active_regions::ActiveRegions;

pub(crate) const STACK_LIMIT: usize = 513;
pub(crate) const TRANSIENT_SIZE: usize = 32;

#[derive(Debug)]
pub(crate) struct Operands {
    pub(crate) stack: [f64; STACK_LIMIT],
    pub(crate) transient: [f64; TRANSIENT_SIZE],
    pub(crate) regions: ActiveRegions,
}

impl Default for Operands {
    fn default() -> Self {
        Self {
            stack: [0.0; STACK_LIMIT],
            transient: [0.0; TRANSIENT_SIZE],
            regions: ActiveRegions::default(),
        }
    }
}
