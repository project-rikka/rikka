use rikka_assist::ASSISTANT;
use rikka_seam::{Request, SeamClient};
use rikka_tray::Tray;
use shell_apps::Apps;
use shell_feeds::{Daemons, Feeds, ScreenRecording};
use shell_kit::taskbar::TaskbarLayout;
use shell_kit::{Presence, TOGGLE};
use shell_notifications::{History, Quiet};

use crate::indicators::Shelf;
use crate::status::Percentage;
use crate::tasks::Opened;
use crate::tray::Fold;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) daemons: Daemons,
    pub(crate) tray: Tray,
    pub(crate) apps: Apps,
    pub(crate) centre: Presence,
    pub(crate) fold: Fold,
    pub(crate) quiet: Quiet,
    pub(crate) history: History,
    pub(crate) recording: ScreenRecording,
    pub(crate) taskbar_layout: TaskbarLayout,
    pub(crate) shelf: Shelf,
    pub(crate) percentage: Percentage,
    pub(crate) feeds: Feeds,
    pub(crate) menu: Opened,
}

impl Shared {
    pub(crate) fn open(&self) {
        self.centre.set(true);
    }

    pub(super) fn assist(&self) {
        self.seam.notify(Request::new(ASSISTANT, TOGGLE));
    }

    pub(super) fn stop_recording(&self) {
        self.seam.notify(self.recording.stop());
    }
}
