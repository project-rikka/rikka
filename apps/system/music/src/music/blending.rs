use std::sync::Arc;
use std::time::Duration;

use music_blend::{Blend, BlendId, Discovered, EntryKey, Outside, Synced};
use music_core::{Catalog, Playlist, Track};
use rikka_app::prelude::*;

use crate::dialogs::{Dialog, Target};
use crate::library::Library;
use crate::music::{Music, Start};
use crate::nav::Place;

const SOON: Duration = Duration::from_millis(1500);
const SURVEY_PAUSE: Duration = Duration::from_secs(2);
const DISCOVER: &str = "music-blend-discover";
const DISCOVER_SOON: &str = "music-blend-discover-soon";

impl Music {
    pub(crate) fn new_blend(&mut self, adding: Arc<[Track]>) {
        let count = self.blends.listed().len();
        let suggested = tr!(
            "music.dialog.suggested-blend",
            count = &(count + 1).to_string()
        );
        self.dialogs.ask(Dialog::Create {
            target: Target::Blend,
            adding,
            suggested: suggested.to_string(),
        });
    }

    pub(crate) fn create_blend(&mut self, name: &str, adding: &[Track]) {
        let mut blend = Blend::new(name);
        blend.add(adding);
        let id = blend.id().clone();
        self.blends.save(blend);
        match adding.is_empty() {
            true => self.go(Place::Blend(id.clone())),
            false => self
                .library
                .show_note(tr!("music.edit.added", name = name).to_string()),
        }
        self.sync_soon(&id);
    }

    pub(crate) fn add_to_blend(&mut self, id: &BlendId, tracks: &[Track]) {
        let mut name = String::new();
        let changed = self.blends.change(id, |blend| {
            name = blend.name().to_owned();
            blend.add(tracks) > 0
        });
        self.library
            .show_note(tr!("music.edit.added", name = name.as_str()).to_string());
        if changed {
            self.sync_soon(id);
        }
    }

    pub(crate) fn remove_from_blend(&mut self, id: &BlendId, key: EntryKey) {
        let mut name = String::new();
        let changed = self.blends.change(id, |blend| {
            name = blend.name().to_owned();
            blend.remove(key)
        });
        if changed {
            self.library
                .show_note(tr!("music.edit.removed", name = name.as_str()).to_string());
            self.sync_soon(id);
        }
    }

    pub(crate) fn rename_blend(&mut self, id: &BlendId, name: &str) {
        if self.blends.change(id, |blend| blend.rename(name)) {
            self.sync_soon(id);
        }
    }

    pub(crate) fn delete_blend(&mut self, id: &BlendId) {
        let Some(blend) = self.blends.find(id) else {
            return;
        };
        cancel(sync_key(id));
        cancel(soon_key(id));
        let connections = self.connections();
        let doomed: Vec<(Arc<dyn Catalog>, Playlist)> = blend
            .mirrors()
            .iter()
            .filter_map(|mirror| {
                let linked = connections
                    .iter()
                    .find(|connection| mirror.belongs_to(connection))?;
                Some((
                    Arc::clone(&linked.catalog),
                    Playlist {
                        id: mirror.playlist.clone(),
                        name: mirror.name.clone(),
                        owner: String::new(),
                        description: String::new(),
                        artwork: Default::default(),
                        length: mirror.tracks.len(),
                        editable: true,
                    },
                ))
            })
            .collect();
        self.blends.delete(id);
        self.nav.forget(&Place::Blend(id.clone()));
        self.arrived();
        if doomed.is_empty() {
            return;
        }
        spawn(
            async move {
                for (catalog, playlist) in doomed {
                    catalog.delete_playlist(&playlist).await?;
                }
                Ok(())
            },
            |library: &mut Library, done: music_core::Result<()>| {
                if let Err(error) = done {
                    library.failed(&error);
                }
            },
        );
    }

    pub(crate) fn blend_outside(&mut self, outside: Outside) {
        match outside {
            Outside::Gone(id) => self.delete_blend(&id),
            Outside::Edited(blend) => {
                let id = blend.id().clone();
                self.blends.follow(Outside::Edited(blend));
                self.sync_soon(&id);
            }
        }
    }

    pub(crate) fn start_blend(&mut self, id: &BlendId, start: Start) {
        let Some(blend) = self.blends.find(id) else {
            return;
        };
        self.player.start(&blend.tracks(), start);
    }

    pub(crate) fn sync_blends(&mut self) {
        cancel(DISCOVER_SOON);
        self.discover();
    }

    pub(crate) fn sync_blend(&mut self, id: &BlendId) {
        let Some(blend) = self.blends.find(id) else {
            return;
        };
        if !self.has_links() || !self.blends.begin_sync(id) {
            return;
        }
        let links = self.connections();
        spawn_keyed(
            sync_key(id),
            async move { music_blend::sync(&blend, &links).await },
            |music: &mut Music, synced: Synced| music.blend_synced(&synced),
        );
    }

    pub(crate) fn blend_synced(&mut self, synced: &Synced) {
        let again = self.blends.end_sync(synced.id());
        self.apply_sync(synced, again);
        if self.blends.discovery_waits() {
            self.discover();
        }
    }

    fn apply_sync(&mut self, synced: &Synced, again: bool) {
        let id = synced.id().clone();
        let Some(current) = self.blends.find(&id) else {
            return;
        };
        if synced.is_gone() && current.is_settled() {
            self.blends.delete(&id);
            self.nav.forget(&Place::Blend(id));
            self.arrived();
            return;
        }
        let mut blend = (*current).clone();
        blend.absorb(synced);
        let is_settled = blend.is_settled();
        if blend != *current {
            self.blends.save(blend);
        }
        self.blends
            .set_account_errors(&id, synced.account_errors().to_vec());
        if again || (!is_settled && synced.account_errors().is_empty()) {
            self.sync_soon(&id);
        }
    }

    pub(crate) fn discover_soon(&mut self) {
        after_keyed(DISCOVER_SOON, SURVEY_PAUSE, |music: &mut Music| {
            music.discover()
        });
    }

    fn discover(&mut self) {
        if !self.has_links() || !self.blends.begin_discovery() {
            return;
        }
        let snapshot = self.blends.snapshot();
        let (links, order) = (self.connections(), self.order());
        spawn_keyed(
            DISCOVER,
            music_blend::discover(snapshot, links, order),
            |music: &mut Music, discovered: Discovered| music.apply_discovered(&discovered),
        );
    }

    pub(crate) fn apply_discovered(&mut self, discovered: &Discovered) {
        self.blends.end_discovery();
        for account_error in discovered.account_errors() {
            tracing::warn!(
                "failed to read Rikka playlists on {} ({}): {}",
                account_error.service.name,
                account_error.account,
                account_error.error
            );
        }
        for blend in discovered.blends() {
            if self.blends.contains(blend.id()) {
                continue;
            }
            self.blends.save(blend.clone());
        }
        for (id, mirror) in discovered.adopted() {
            self.blends.change(id, |blend| blend.adopt(mirror));
        }
        for id in self.blends.ids() {
            self.sync_blend(&id);
        }
    }

    fn sync_soon(&mut self, id: &BlendId) {
        let id = id.clone();
        after_keyed(soon_key(&id), SOON, move |music: &mut Music| {
            music.sync_blend(&id);
        });
    }
}

fn sync_key(id: &BlendId) -> (&'static str, BlendId) {
    ("music-blend-sync", id.clone())
}

fn soon_key(id: &BlendId) -> (&'static str, BlendId) {
    ("music-blend-soon", id.clone())
}
