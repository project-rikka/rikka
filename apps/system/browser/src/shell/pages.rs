use browser_store::{ArchivedTabId, Restored, Timestamp};
use rikka_app::prelude::*;

use crate::internal::Span;
use crate::shell::Shell;
use browser_model::{Placement, ProfileId, ProfileKey, TabId};
use browser_pages::{Internal, Section};

impl Shell {
    pub(crate) fn prepare_page(&mut self, active: Option<TabId>, partners: &[TabId]) {
        self.prepare_home(active, partners);
        let internal = active.and_then(|tab| (*self.model().tab(tab)?.internal).clone());
        if internal == self.page_shown {
            return;
        }
        self.page_shown = internal.clone();
        let writes = self.model().writes(self.id);
        let store = self.browser.store();
        let key = self.model().profile_here(self.id);
        match internal {
            Some(Internal::History) => {
                let history = self.browser.engine(key).map(|runtime| runtime.history());
                self.pages.history.open(history);
            }
            Some(Internal::Archive) => self.pages.archive.open(writes.then_some(store)),
            Some(Internal::Settings(Some(Section::ClearData))) => self.pages.clearing.reset(),
            Some(Internal::Settings(Some(Section::Chrome))) => self.read_privacy(key),
            Some(Internal::Downloads) => {
                if let Err(error) = self.browser.engine(key).map(|runtime| runtime.downloads()) {
                    tracing::warn!(%error, "failed to start Chrome for downloads");
                }
            }
            Some(Internal::Extensions | Internal::Settings(Some(Section::Extensions))) => {
                self.list_extensions();
            }
            Some(Internal::Home | Internal::Settings(_) | Internal::Missing(_)) | None => {}
        }
    }

    pub(crate) fn history_changed(&mut self) {
        skip_view();
        if self.page_shown == Some(Internal::History) {
            self.pages.history.changed();
        }
    }

    pub(crate) fn persistent_profile(&self) -> Option<ProfileId> {
        self.model().profile_here(self.id).persistent_id()
    }

    pub(crate) fn open_page(&mut self, url: String) {
        {
            let mut model = self.browser.edit();
            let active = model.window(self.id).and_then(|layout| layout.active());
            let placement = active.map_or(Placement::Top, Placement::After);
            model.open_tab(self.id, Some(url), placement, true);
        }
        self.address.end();
        self.changed();
    }

    pub(crate) fn open_aside(&mut self, url: String) {
        let opened = {
            let mut model = self.browser.edit();
            let active = model.window(self.id).and_then(|layout| layout.active());
            let placement = active.map_or(Placement::Top, Placement::After);
            model.open_tab(self.id, Some(url), placement, false)
        };
        if let Some(tab) = opened
            && let Ok(Some(view)) = self.browser.activate(tab)
        {
            view.set_visible(false);
        }
        self.changed();
    }

    pub(crate) fn restore_archived(&mut self, id: ArchivedTabId) {
        let Some(space) = self
            .model()
            .window(self.id)
            .filter(|layout| !layout.private)
            .and_then(|layout| layout.space())
        else {
            return;
        };
        let store = self.browser.store();
        spawn_blocking(
            move || store.restore_archived(id),
            move |shell: &mut Shell, restored: browser_store::Result<Option<Restored>>| {
                shell.pages.archive.drop_entry(id);
                match restored {
                    Ok(Some(restored)) => {
                        let restored = Restored { space, ..restored };
                        shell.reopened(restored);
                    }
                    Ok(None) => {}
                    Err(error) => tracing::warn!(%error, "failed to restore archived tab"),
                }
            },
        );
    }

    pub(crate) fn clear_history(&mut self, span: Span) {
        self.pages.history.clear(span);
    }

    pub(crate) fn clear_browsing_data(&mut self) {
        let Some(profile) = self.persistent_profile() else {
            return;
        };
        let key = ProfileKey::Persistent(profile);
        let (span, kinds) = self.pages.clearing.chosen();
        self.pages.clearing.begin();
        self.browser.store().clear_browsing_data(
            profile,
            kinds.store(),
            span.range(Timestamp::now()),
        );
        let web = kinds.web();
        if web == rikka_web::DataKinds::default() {
            self.pages.clearing.finish();
            return;
        }
        let storage = match self.browser.engine(key) {
            Ok(engine) => engine.storage(),
            Err(error) => {
                tracing::warn!(%error, "failed to clear browsing data");
                self.pages.clearing.finish();
                return;
            }
        };
        spawn(
            storage.clear_since(span.since(), web),
            |shell: &mut Shell, cleared: rikka_web::Result<()>| {
                if let Err(error) = cleared {
                    tracing::warn!(%error, "failed to clear browsing data");
                }
                shell.pages.clearing.finish();
            },
        );
    }

    pub(crate) fn delete_profile(&mut self, profile: ProfileId) {
        if !self.model().removable_profile(profile) {
            return;
        }
        self.browser.forget_engine(profile);
        let store = self.browser.store();
        spawn_blocking(
            move || {
                rikka_web::wait_for_closed_runtimes();
                store.remove_profile(profile)?;
                match crate::engines::folder(profile) {
                    Ok(folder) => {
                        if let Err(error) = std::fs::remove_dir_all(&folder)
                            && error.kind() != std::io::ErrorKind::NotFound
                        {
                            tracing::warn!(%error, folder = %folder.display(), "failed to remove profile folder");
                        }
                    }
                    Err(error) => tracing::warn!(%error, "failed to locate profile folder"),
                }
                Ok(())
            },
            move |shell: &mut Shell, removed: browser_store::Result<()>| match removed {
                Ok(()) => {
                    shell.browser.edit().forget_profile(profile);
                    shell.changed();
                }
                Err(error) => tracing::warn!(%error, "failed to remove profile"),
            },
        );
    }
}
