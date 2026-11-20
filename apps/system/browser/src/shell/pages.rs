use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use browser_pages::Cause;
use browser_store::{ArchivedTabId, Restored, Timestamp};
use rikka_app::prelude::*;

use crate::internal::Span;
use crate::shell::Shell;
use crate::shell::sync::activate;
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
        match internal {
            Some(Internal::History) => {
                let profile = self.persistent_profile();
                self.pages.history.open(store, profile);
            }
            Some(Internal::Archive) => self.pages.archive.open(writes.then_some(store)),
            Some(Internal::Settings(Some(Section::ClearData))) => self.pages.clearing.reset(),
            Some(Internal::Settings(Some(Section::Content | Section::Network))) => {
                let key = self.model().profile_here(self.id);
                let settings = self.browser.engine_settings(key);
                self.sections.web.reset(&settings);
            }
            Some(Internal::Settings(Some(Section::Sites))) => {
                let profile = self.persistent_profile();
                self.pages.sites.open(store, profile);
            }
            Some(Internal::Downloads) => self.load_downloads(),
            Some(Internal::Source(page)) => {
                let key = self.model().profile_here(self.id);
                self.pages.source.open(&self.browser, key, page);
            }
            Some(Internal::Extensions | Internal::Settings(Some(Section::Extensions))) => {
                self.list_extensions();
            }
            Some(Internal::Home | Internal::Settings(_) | Internal::Missing(_)) | None => {}
        }
    }

    pub(crate) fn persistent_profile(&self) -> Option<ProfileId> {
        self.model().profile_here(self.id).persistent_id()
    }

    pub(crate) fn open_aside(&mut self, url: String) {
        self.open_aside_from(None, url);
    }

    pub(crate) fn open_link_aside(&mut self, opener: TabId, url: String) {
        self.open_aside_from(Some(opener), url);
    }

    fn open_aside_from(&mut self, opener: Option<TabId>, url: String) {
        let opened = {
            let mut model = self.browser.edit();
            let active = model.window(self.id).and_then(|layout| layout.active());
            let placement = active.map_or(Placement::Top, Placement::After);
            let opened = model.open_tab(self.id, Some(url), placement, false);
            if let Some(tab_entry) = opened.and_then(|tab| model.tab_mut(tab)) {
                tab_entry.visit_tracker.set_cause(Cause::Link);
            }
            opened
        };
        if let (Some(tab), Some(opener)) = (opened, opener) {
            self.refer(tab, opener);
        }
        if let Some(tab) = opened
            && let Ok(Some((view, activation))) = self.browser.activate(tab)
        {
            view.set_visible(false);
            activate(&view, activation, self.referred_load(tab));
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
        self.pages.history.clear(span.range(Timestamp::now()));
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
            kinds.to_data_kinds(),
            span.range(Timestamp::now()),
        );
        if kinds.site_settings {
            self.reload_zooms(profile);
        }
        if kinds.history && span.length().is_none() {
            for (owner, session) in self.browser.live_sessions() {
                if owner == key {
                    session.purge_history();
                }
            }
        }
        let web = kinds.web();
        if web == rikka_web::ClearFlags::NONE {
            self.pages.clearing.finish();
            return;
        }
        let storage = match self.browser.engine(key) {
            Ok(engine) => engine.storage(),
            Err(error) => {
                tracing::warn!(%error, "failed to clear site data");
                self.pages.clearing.finish();
                return;
            }
        };
        spawn(
            async move {
                match span.length() {
                    None => storage.clear(web).await,
                    Some(length) => {
                        let to = SystemTime::now();
                        let from = to.checked_sub(length).unwrap_or(UNIX_EPOCH);
                        storage.clear_between(from, to, web).await
                    }
                }
            },
            |shell: &mut Shell, cleared: rikka_web::Result<()>| {
                if let Err(error) = cleared {
                    tracing::warn!(%error, "failed to clear site data");
                }
                shell.pages.clearing.finish();
            },
        );
    }

    fn reload_zooms(&mut self, profile: ProfileId) {
        let store = self.browser.store();
        spawn_blocking(
            move || {
                store.flush()?;
                store.sites_with_settings(profile)
            },
            move |shell: &mut Shell,
                  sites: browser_store::Result<Vec<browser_store::SiteSettings>>| {
                let Ok(sites) = sites else {
                    return;
                };
                let hosts: HashMap<String, f64> = sites
                    .into_iter()
                    .filter_map(|site| Some((site.host, site.zoom?)))
                    .collect();
                shell.browser.edit().zooms.replace(profile, hosts);
                shell.refit_zoom();
            },
        );
    }

    pub(crate) fn refit_zoom(&mut self) {
        let tabs = self
            .model()
            .window(self.id)
            .map(|layout| layout.tabs())
            .unwrap_or_default();
        for tab in tabs {
            self.fit_zoom(tab);
        }
    }

    pub(crate) fn prune_history(&mut self) {
        let Some(retention) = self.browser.preferences().history_kept().span() else {
            return;
        };
        let oldest = Timestamp::now().saturating_sub(retention);
        let model = self.model();
        for profile in model.profiles.list() {
            model.store().prune_history(profile.id, oldest);
        }
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
                for folder in crate::engines::folders(profile) {
                    if let Err(error) = std::fs::remove_dir_all(&folder)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        tracing::warn!(%error, folder = %folder.display(), "failed to remove profile folder");
                    }
                }
                Ok(())
            },
            move |shell: &mut Shell, removed: browser_store::Result<()>| match removed {
                Ok(()) => {
                    shell.browser.services().engine_settings.forget(profile);
                    shell.browser.edit().forget_profile(profile);
                    shell.changed();
                }
                Err(error) => tracing::warn!(%error, "failed to remove profile"),
            },
        );
    }
}
