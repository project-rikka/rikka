use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use rikka_notifications::{Notification, Timeout, Urgency};
use rikka_seam::SeamClient;
use shell_apps::Apps;

use crate::action_router::ActionRouter;
use crate::history::History;
use crate::quiet::Quiet;
use crate::stack::Stack;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) stack: Stack,
    pub(crate) history: History,
    pub(super) action_router: ActionRouter,
    pub(crate) preferences: Preferences,
    pub(crate) apps: Apps,
}

#[derive(Clone, Debug)]
pub(crate) struct Preferences {
    quiet: Quiet,
    linger: Rc<Cell<Duration>>,
}

impl Preferences {
    #[must_use]
    pub(crate) fn new(quiet: Quiet, linger: Duration) -> Preferences {
        Preferences {
            quiet,
            linger: Rc::new(Cell::new(linger)),
        }
    }

    pub(super) fn stand_for(&self, linger: Duration) {
        self.linger.set(linger);
    }

    #[must_use]
    pub(super) fn keeps_out(&self, urgency: Urgency) -> bool {
        self.quiet.get() && urgency < Urgency::Critical
    }

    #[must_use]
    pub(crate) fn linger(&self, notification: &Notification) -> Option<Duration> {
        match (notification.timeout, notification.urgency) {
            (Timeout::Default, Urgency::Critical) => None,
            (timeout, _) => timeout.against(self.linger.get()),
        }
    }
}
