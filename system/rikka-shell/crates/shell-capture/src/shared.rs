use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use rikka_seam::SeamClient;
use shell_feeds::ScreenRecording;
use shell_kit::Presence;

use crate::aim::Aimed;
use crate::frozen::Frozen;
use crate::scan::Codes;
use crate::selection::Selection;
use crate::windows::Windows;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) aimed: Aimed,
    pub(crate) selection: Selection,
    pub(crate) windows: Windows,
    pub(crate) recording: ScreenRecording,
    pub(crate) frozen: Rc<RefCell<Option<Arc<Frozen>>>>,
    pub(crate) picking: Presence,
    pub(super) codes: Codes,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, recording: ScreenRecording) -> Shared {
        Shared {
            seam,
            aimed: Aimed::default(),
            selection: Selection::default(),
            windows: Windows::default(),
            recording,
            frozen: Rc::default(),
            picking: Presence::default(),
            codes: Codes::default(),
        }
    }
}
