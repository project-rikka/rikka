use std::cell::RefCell;
use std::collections::HashMap;

use browser_bookmarks::Mirror;
use rikka_ui::Revision;
use rikka_web::{Account, ChromeSettingValue, Exit, ExtensionInfo, Failure, PrivacySetting};

use browser_model::ProfileKey;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Phase {
    #[default]
    Starting,
    Ready,
    Failed(Failure),
    Exited(Exit),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Syncing {
    #[default]
    Idle,
    Running,
    Again,
}

#[derive(Debug, Default)]
struct Engine {
    phase: Phase,
    account: Option<Account>,
    extensions: Option<Vec<ExtensionInfo>>,
    privacy: HashMap<PrivacySetting, ChromeSettingValue>,
    syncing: Syncing,
    synced: Option<Mirror>,
}

#[derive(Debug, Default)]
struct Engines {
    by_profile: HashMap<ProfileKey, Engine>,
    revision: Revision,
}

#[derive(Debug, Default)]
pub(crate) struct Chrome {
    engines: RefCell<Engines>,
}

impl Chrome {
    fn read<T>(&self, key: ProfileKey, read: impl FnOnce(&Engine) -> T) -> Option<T> {
        let engines = self.engines.borrow();
        engines.revision.read();
        engines.by_profile.get(&key).map(read)
    }

    fn edit<T>(&self, key: ProfileKey, edit: impl FnOnce(&mut Engine) -> T) -> T {
        let mut engines = self.engines.borrow_mut();
        engines.revision.changed();
        edit(engines.by_profile.entry(key).or_default())
    }

    pub(crate) fn started(&self, key: ProfileKey) {
        self.edit(key, |engine| *engine = Engine::default());
    }

    pub(crate) fn forget(&self, key: ProfileKey) {
        let mut engines = self.engines.borrow_mut();
        if engines.by_profile.remove(&key).is_some() {
            engines.revision.changed();
        }
    }

    pub(crate) fn phase(&self, key: ProfileKey) -> Option<Phase> {
        self.read(key, |engine| engine.phase.clone())
    }

    pub(crate) fn is_ready(&self, key: ProfileKey) -> bool {
        self.phase(key) == Some(Phase::Ready)
    }

    pub(crate) fn set_phase(&self, key: ProfileKey, phase: Phase) {
        self.edit(key, |engine| engine.phase = phase);
    }

    pub(crate) fn account(&self, key: ProfileKey) -> Option<Account> {
        self.read(key, |engine| engine.account.clone()).flatten()
    }

    pub(crate) fn set_account(&self, key: ProfileKey, account: Option<Account>) {
        self.edit(key, |engine| engine.account = account);
    }

    pub(crate) fn extensions(&self, key: ProfileKey) -> Option<Vec<ExtensionInfo>> {
        self.read(key, |engine| engine.extensions.clone()).flatten()
    }

    pub(crate) fn set_extensions(&self, key: ProfileKey, extensions: Vec<ExtensionInfo>) {
        self.edit(key, |engine| engine.extensions = Some(extensions));
    }

    pub(crate) fn privacy(
        &self,
        key: ProfileKey,
        setting: PrivacySetting,
    ) -> Option<ChromeSettingValue> {
        self.read(key, |engine| engine.privacy.get(&setting).copied())
            .flatten()
    }

    pub(crate) fn set_privacy(
        &self,
        key: ProfileKey,
        setting: PrivacySetting,
        value: ChromeSettingValue,
    ) {
        self.edit(key, |engine| engine.privacy.insert(setting, value));
    }

    pub(crate) fn begin_sync(&self, key: ProfileKey) -> bool {
        let mut engines = self.engines.borrow_mut();
        let engine = engines.by_profile.entry(key).or_default();
        match engine.syncing {
            Syncing::Idle => {
                engine.syncing = Syncing::Running;
                true
            }
            Syncing::Running | Syncing::Again => {
                engine.syncing = Syncing::Again;
                false
            }
        }
    }

    pub(crate) fn end_sync(&self, key: ProfileKey, synced: Option<Mirror>) -> bool {
        let mut engines = self.engines.borrow_mut();
        let Some(engine) = engines.by_profile.get_mut(&key) else {
            return false;
        };
        if synced.is_some() {
            engine.synced = synced;
        }
        std::mem::take(&mut engine.syncing) == Syncing::Again
    }

    pub(crate) fn needs_sync(&self, key: ProfileKey, mirror: &Mirror) -> bool {
        let engines = self.engines.borrow();
        engines.by_profile.get(&key).is_some_and(|engine| {
            engine.phase == Phase::Ready
                && engine.syncing == Syncing::Idle
                && engine.synced.as_ref() != Some(mirror)
        })
    }
}
