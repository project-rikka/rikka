use std::cell::RefCell;
use std::rc::Rc;

use rikka_color_formats::Rgb;
use rikka_seam::SeamClient;
use shell_kit::{Presence, Recents, Spot};

use crate::picker::Frame;
use crate::preferences::Preferences;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) preferences: Preferences,
    pub(crate) history: Recents<Rgb>,
    pub(super) frames: Rc<RefCell<Vec<Frame>>>,
    pub(crate) picking: Presence,
    pub(crate) editor: Presence,
    pub(crate) spot: Spot,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, preferences: Preferences, history: Recents<Rgb>) -> Shared {
        Shared {
            seam,
            preferences,
            history,
            frames: Rc::default(),
            picking: Presence::default(),
            editor: Presence::default(),
            spot: Spot::default(),
        }
    }

    #[must_use]
    pub(crate) fn frame(&self, output: &str) -> Option<Frame> {
        let frames = self.frames.borrow();
        frames
            .iter()
            .find(|frame| frame.output == output)
            .or_else(|| frames.first().filter(|_| frames.len() == 1))
            .cloned()
    }
}
