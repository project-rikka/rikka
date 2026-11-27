use std::time::Duration;

use browser_bookmarks::{Mirror, Plan};
use browser_store::BookmarkSync;
use rikka_app::prelude::*;
use rikka_web::BookmarkNode;

use crate::shell::Shell;
use browser_model::{ProfileId, ProfileKey};

const READING: &str = "rikka-browser-bookmarks-read";
const WRITING: &str = "rikka-browser-bookmarks-write";
const SETTLE: &str = "rikka-browser-bookmarks-settle";
const SETTLE_DELAY: Duration = Duration::from_secs(1);

type Read = Result<(Vec<BookmarkNode>, Option<BookmarkSync>), String>;

impl Shell {
    pub(crate) fn sync_bookmarks(&mut self, profile: ProfileId) {
        let key = ProfileKey::Persistent(profile);
        let chrome = &self.browser().services().chrome;
        if !chrome.is_ready(key) {
            return;
        }
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        if !chrome.begin_sync(key) {
            return;
        }
        let bookmarks = runtime.bookmarks();
        let store = self.browser().store();
        spawn_keyed(
            (READING, profile),
            async move {
                let tree = bookmarks.tree().await.map_err(|error| error.to_string())?;
                store.flush().map_err(|error| error.to_string())?;
                let synced = store
                    .bookmark_sync(profile)
                    .map_err(|error| error.to_string())?;
                Ok((tree, synced))
            },
            move |shell: &mut Shell, read: Read| shell.bookmarks_read(profile, read),
        );
    }

    fn bookmarks_read(&mut self, profile: ProfileId, read: Read) {
        let key = ProfileKey::Persistent(profile);
        let (tree, synced) = match read {
            Ok(read) => read,
            Err(error) => {
                tracing::warn!(%error, "failed to read Chrome bookmarks");
                self.bookmarks_synced(profile, None);
                return;
            }
        };
        let mirror = self.model().bookmark_mirror(profile);
        let plan = match Plan::new(&tree, &mirror, synced.as_ref()) {
            Ok(plan) => plan,
            Err(error) => {
                tracing::warn!(%error, "failed to plan the bookmark sync");
                self.bookmarks_synced(profile, None);
                return;
            }
        };
        let pending = self.browser().edit().apply_bookmarks(profile, plan);
        let applied = self.model().bookmark_mirror(profile);
        self.changed();
        let Some(runtime) = self.browser().running_engine(key) else {
            self.bookmarks_synced(profile, None);
            return;
        };
        let bookmarks = runtime.bookmarks();
        spawn_keyed(
            (WRITING, profile),
            async move { pending.apply_remote(&bookmarks).await },
            move |shell: &mut Shell, written: Result<BookmarkSync, browser_bookmarks::Error>| {
                match written {
                    Ok(synced) => {
                        shell.browser().store().save_bookmark_sync(profile, synced);
                        shell.bookmarks_synced(profile, Some(applied.clone()));
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to write Chrome bookmarks");
                        shell.bookmarks_synced(profile, None);
                    }
                }
            },
        );
    }

    fn bookmarks_synced(&mut self, profile: ProfileId, applied: Option<Mirror>) {
        let key = ProfileKey::Persistent(profile);
        let again = self.browser().services().chrome.end_sync(key, applied);
        if again {
            self.sync_bookmarks(profile);
        }
    }

    pub(crate) fn settle_bookmarks(&mut self) {
        after_keyed(SETTLE, SETTLE_DELAY, |shell: &mut Shell| {
            skip_view();
            let profiles: Vec<ProfileId> = shell
                .model()
                .profiles
                .list()
                .iter()
                .map(|profile| profile.id)
                .collect();
            for profile in profiles {
                let mirror = shell.model().bookmark_mirror(profile);
                let key = ProfileKey::Persistent(profile);
                if shell.browser().services().chrome.needs_sync(key, &mirror) {
                    shell.sync_bookmarks(profile);
                }
            }
        });
    }
}
