use std::cell::RefCell;
use std::collections::HashMap;

use browser_engine_settings::EngineSettings;
use browser_store::Store;
use rikka_ui::Observers;
use rikka_web::{Locales, Preferences, Session};

use crate::browser::Browser;
use browser_model::{ProfileId, ProfileKey};

#[derive(Debug, Default)]
pub(crate) struct EngineBook {
    profiles: RefCell<HashMap<ProfileId, EngineSettings>>,
    interface: RefCell<String>,
    observers: Observers,
}

impl EngineBook {
    pub(crate) fn load(&self, store: &Store, profiles: impl IntoIterator<Item = ProfileId>) {
        let mut loaded = self.profiles.borrow_mut();
        for profile in profiles {
            match store.profile_settings::<EngineSettings>(profile) {
                Ok(Some(settings)) => {
                    loaded.insert(profile, settings);
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "failed to read profile settings"),
            }
        }
    }

    fn get(&self, profile: ProfileId) -> EngineSettings {
        self.observers.read();
        self.profiles
            .borrow()
            .get(&profile)
            .cloned()
            .unwrap_or_default()
    }

    fn set(&self, profile: ProfileId, settings: EngineSettings) {
        self.profiles.borrow_mut().insert(profile, settings);
        self.observers.changed();
    }

    fn adopt_interface(&self, interface: &str) -> bool {
        let mut adopted = self.interface.borrow_mut();
        if *adopted == interface {
            return false;
        }
        interface.clone_into(&mut adopted);
        true
    }

    pub(crate) fn forget(&self, profile: ProfileId) {
        if self.profiles.borrow_mut().remove(&profile).is_some() {
            self.observers.changed();
        }
    }
}

fn follow(settings: &EngineSettings, session: &Session) {
    let current = session.settings();
    let followed = settings.session(current.clone());
    if followed != current {
        session.set_settings(followed);
    }
}

fn interface_locale() -> String {
    rikka_i18n::current_locale().to_string()
}

impl Browser {
    pub(crate) fn settings_profile(&self, key: ProfileKey) -> Option<ProfileId> {
        match key {
            ProfileKey::Persistent(profile) => Some(profile),
            ProfileKey::Private => self
                .model()
                .profiles
                .list()
                .first()
                .map(|profile| profile.id),
        }
    }

    pub(crate) fn engine_settings(&self, key: ProfileKey) -> EngineSettings {
        self.settings_profile(key)
            .map(|profile| self.services().engine_settings.get(profile))
            .unwrap_or_default()
    }

    pub(crate) fn web_preferences(&self, key: ProfileKey) -> Preferences {
        self.engine_settings(key)
            .preferences()
            .https_only(crate::https::mode(self.preferences().https_only()))
    }

    pub(crate) fn web_locales(&self, key: ProfileKey) -> Locales {
        self.engine_settings(key).locales(&interface_locale())
    }

    pub(crate) fn follow_interface_locale(&self) {
        if !self
            .services()
            .engine_settings
            .adopt_interface(&interface_locale())
        {
            return;
        }
        for (key, runtime) in self.live_engines() {
            runtime.set_locales(&self.web_locales(key));
        }
    }

    pub(crate) fn dress(&self, key: ProfileKey, session: &Session) {
        let settings = self.engine_settings(key);
        follow(&settings, session);
        session.set_user_content(settings.dress(session.user_content()));
    }

    pub(crate) fn change_engine_settings(
        &self,
        profile: ProfileId,
        change: impl FnOnce(&mut EngineSettings),
    ) {
        let book = &self.services().engine_settings;
        let before = book.get(profile);
        let mut after = before.clone();
        change(&mut after);
        if after == before {
            return;
        }
        if let Err(error) = self.store().set_profile_settings(profile, &after) {
            tracing::warn!(%error, "failed to save profile settings");
        }
        book.set(profile, after.clone());
        let follows = |key: ProfileKey| self.settings_profile(key) == Some(profile);
        for (key, runtime) in self.live_engines() {
            if !follows(key) {
                continue;
            }
            runtime.set_preferences(&self.web_preferences(key));
            if after.page_languages != before.page_languages {
                runtime.set_locales(&self.web_locales(key));
            }
        }
        let restyled = after.user_style != before.user_style;
        let protected = after.tracking_protection != before.tracking_protection;
        for (key, session) in self.live_sessions() {
            if !follows(key) {
                continue;
            }
            if protected {
                follow(&after, &session);
            }
            if restyled {
                session.set_user_content(after.dress(session.user_content()));
            }
        }
    }

    pub(crate) fn live_sessions(&self) -> Vec<(ProfileKey, Session)> {
        let model = self.model();
        model
            .windows
            .iter()
            .flat_map(|layout| layout.tabs())
            .filter_map(|tab| {
                let tab_entry = model.tab(tab)?;
                Some((tab_entry.profile, tab_entry.page().view()?.clone()))
            })
            .collect()
    }
}
