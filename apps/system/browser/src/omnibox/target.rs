use browser_store::ArchivedTabId;

use crate::command::Command;
use browser_model::{Slot, SpaceId, TabId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Target {
    Tab(TabId),
    Slot(Slot, Option<SpaceId>),
    Archived(ArchivedTabId),
    Act(Act),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Act {
    Run(Command),
    NewSpace,
    Space(SpaceId),
    Archive,
    CopyAddress,
}
