use std::time::Duration;

use crate::Presence;

#[derive(Clone, Debug)]
pub(super) struct PresenceBinding {
    presence: Presence,
    exit: Option<Duration>,
}

impl PresenceBinding {
    #[must_use]
    pub(super) fn at_once(presence: Presence) -> PresenceBinding {
        PresenceBinding {
            presence,
            exit: None,
        }
    }

    #[must_use]
    pub(crate) fn playing(presence: Presence, exit: Duration) -> PresenceBinding {
        PresenceBinding {
            presence,
            exit: Some(exit),
        }
    }

    #[must_use]
    pub(crate) fn presence(&self) -> &Presence {
        &self.presence
    }

    #[must_use]
    pub(crate) fn exit(&self) -> Option<Duration> {
        self.exit
    }
}
