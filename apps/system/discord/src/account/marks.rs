use std::cell::RefCell;
use std::collections::HashMap;

use discord_core::{Area, ChannelId};
use rikka_ui::Revision;

#[derive(Debug, Default)]
pub(crate) struct Marks {
    everything: Revision,
    areas: RefCell<HashMap<Area, Revision>>,
}

impl Marks {
    pub(crate) fn read(&self, area: Area) {
        self.everything.read();
        if area == Area::Everything {
            return;
        }
        self.areas.borrow_mut().entry(area).or_default().read();
    }

    pub(crate) fn invalidate(&mut self, changed: &[Area]) {
        if changed.contains(&Area::Everything) {
            self.everything.changed();
            self.areas.get_mut().clear();
            return;
        }
        let areas = self.areas.get_mut();
        for area in changed {
            if let Some(revision) = areas.get_mut(area) {
                revision.changed();
            }
        }
    }

    pub(crate) fn forget(&mut self, channel: &ChannelId) {
        self.areas
            .get_mut()
            .retain(|area, _| area.channel() != Some(channel));
    }
}
