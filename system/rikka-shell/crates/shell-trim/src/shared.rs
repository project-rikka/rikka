use rikka_seam::SeamClient;
use shell_kit::{Spot, Waiting};

use crate::opening::Opening;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) waiting: Waiting<Opening>,
    pub(crate) spot: Spot,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, waiting: Waiting<Opening>) -> Shared {
        Shared {
            seam,
            waiting,
            spot: Spot::default(),
        }
    }
}
