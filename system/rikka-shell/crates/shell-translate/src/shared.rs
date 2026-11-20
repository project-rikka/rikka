use std::rc::Rc;

use rikka_seam::SeamClient;
use shell_capture::Snip;
use shell_kit::Spot;

use crate::choices::Choices;
use crate::prompts::Prompts;

#[derive(Clone)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) prompts: Rc<Prompts>,
    pub(crate) choices: Choices,
    pub(crate) snip: Snip,
    pub(crate) spot: Spot,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, prompts: Prompts, choices: Choices, snip: Snip) -> Shared {
        Shared {
            seam,
            prompts: Rc::new(prompts),
            choices,
            snip,
            spot: Spot::default(),
        }
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}
