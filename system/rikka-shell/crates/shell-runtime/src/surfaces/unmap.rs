use std::collections::HashMap;

use super::Surfaces;
use shell_kit::Presence;
use shell_kit::hosting::UnmapCallback;

#[derive(Clone, Copy, Debug)]
pub(crate) struct UnmapSync(u64);

impl rikka_wayland_client::Key for UnmapSync {
    fn into_raw(self) -> u64 {
        self.0
    }

    fn from_raw(raw: u64) -> UnmapSync {
        UnmapSync(raw)
    }
}

#[derive(Default)]
pub(super) struct UnmapCallbacks {
    next: u64,
    pending: HashMap<u64, Vec<UnmapCallback>>,
}

impl UnmapCallbacks {
    fn insert(&mut self, callbacks: Vec<UnmapCallback>) -> UnmapSync {
        let token = self.next;
        self.next += 1;
        self.pending.insert(token, callbacks);
        UnmapSync(token)
    }

    fn take(&mut self, sync: UnmapSync) -> Vec<UnmapCallback> {
        self.pending.remove(&sync.0).unwrap_or_default()
    }
}

impl Surfaces {
    pub(crate) fn follow_unmaps(&mut self) {
        let unmapped: Vec<Presence> = self
            .descs
            .iter()
            .filter_map(|(_, surface)| surface.settings.presence())
            .filter(|presence| presence.has_unmap_callbacks() && self.is_unmapped(presence))
            .cloned()
            .collect();
        for presence in unmapped {
            let callbacks = presence.take_unmap_callbacks();
            if callbacks.is_empty() {
                continue;
            }
            let sync = self.unmap_callbacks.insert(callbacks);
            self.host.display().sync(&self.route).set_key(sync);
        }
    }

    fn is_unmapped(&self, presence: &Presence) -> bool {
        self.descs
            .iter()
            .filter(|(_, surface)| {
                surface
                    .settings
                    .presence()
                    .is_some_and(|bound| bound.shares(presence))
            })
            .all(|(desc, surface)| {
                !surface.settings.is_wanted()
                    && !surface.is_leaving()
                    && !self.spawned_anywhere(desc)
            })
    }

    pub(crate) fn take_unmap_callbacks(&mut self, sync: UnmapSync) -> Vec<UnmapCallback> {
        self.unmap_callbacks.take(sync)
    }
}
