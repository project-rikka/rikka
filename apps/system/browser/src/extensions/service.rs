use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, VecDeque};

use rikka_ui::Revision;
use rikka_web::{
    ActionKind, Extension, ExtensionAction, InstallRequest, OptionalPermissionsRequest,
    UpdateRequest,
};

use crate::browser::Browser;
use browser_model::{ProfileKey, TabId, WindowId};

#[derive(Debug)]
pub(crate) enum Ask {
    Install(InstallRequest),
    Update(UpdateRequest),
    Permissions(OptionalPermissionsRequest),
}

#[derive(Debug)]
pub(crate) struct Asking {
    pub(crate) window: WindowId,
    pub(crate) key: ProfileKey,
    pub(crate) ask: Ask,
}

pub(crate) type ActionKey = (String, ActionKind);

#[derive(Debug, Default)]
pub(crate) struct ExtensionState {
    pub(crate) installed: HashMap<ProfileKey, Vec<Extension>>,
    pub(crate) defaults: HashMap<(ProfileKey, ActionKey), ExtensionAction>,
    pub(crate) tabs: HashMap<(TabId, ActionKey), ExtensionAction>,
    pub(crate) asking: VecDeque<Asking>,
    pub(crate) failed: Option<String>,
    revision: Revision,
}

#[derive(Debug, Default)]
pub(crate) struct Extensions {
    state: RefCell<ExtensionState>,
}

impl Browser {
    pub(crate) fn extensions(&self) -> &Extensions {
        &self.services().extensions
    }
}

impl Extensions {
    pub(crate) fn state(&self) -> Ref<'_, ExtensionState> {
        let state = self.state.borrow();
        state.revision.read();
        state
    }

    pub(crate) fn edit(&self) -> RefMut<'_, ExtensionState> {
        let mut state = self.state.borrow_mut();
        state.revision.changed();
        state
    }

    pub(crate) fn installed(&self, key: ProfileKey) -> Vec<Extension> {
        self.state()
            .installed
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn listed(&self, key: ProfileKey, extensions: Vec<Extension>) {
        self.edit().installed.insert(key, extensions);
    }

    pub(crate) fn changed(&self, key: ProfileKey, extension: Extension, gone: bool) {
        let mut state = self.edit();
        let id = extension.web_extension_id.clone();
        if gone {
            state
                .defaults
                .retain(|(profile, (owner, _)), _| *profile != key || *owner != id);
            state.tabs.retain(|(_, (owner, _)), _| *owner != id);
        }
        let installed = state.installed.entry(key).or_default();
        let at = installed
            .iter()
            .position(|candidate| candidate.web_extension_id == id);
        match (at, gone) {
            (Some(at), true) => {
                installed.remove(at);
            }
            (Some(at), false) => installed[at] = extension,
            (None, false) => installed.push(extension),
            (None, true) => {}
        }
    }

    pub(crate) fn act(&self, key: ProfileKey, action: ActionKey, value: ExtensionAction) {
        self.edit().defaults.insert((key, action), value);
    }

    pub(crate) fn act_on(&self, tab: TabId, action: ActionKey, value: ExtensionAction) {
        self.edit().tabs.insert((tab, action), value);
    }

    pub(crate) fn ask(&self, asking: Asking) {
        self.edit().asking.push_back(asking);
    }

    pub(crate) fn take_asking(&self, window: WindowId) -> Option<Asking> {
        let mut state = self.edit();
        let at = state
            .asking
            .iter()
            .position(|asking| asking.window == window)?;
        state.asking.remove(at)
    }

    pub(crate) fn fail(&self, failure: Option<String>) {
        self.edit().failed = failure;
    }

    pub(crate) fn forget(&self, key: ProfileKey) {
        let mut state = self.edit();
        state.installed.remove(&key);
        state.defaults.retain(|(profile, _), _| *profile != key);
        state.asking.retain(|asking| asking.key != key);
    }

    pub(crate) fn retain_tabs(&self, alive: impl Fn(TabId) -> bool) {
        let has_closed = self.state.borrow().tabs.keys().any(|(tab, _)| !alive(*tab));
        if has_closed {
            self.edit().tabs.retain(|(tab, _), _| alive(*tab));
        }
    }
}

pub(crate) fn merged(base: &ExtensionAction, over: Option<&ExtensionAction>) -> ExtensionAction {
    let Some(over) = over else {
        return base.clone();
    };
    ExtensionAction {
        title: over.title.clone().or_else(|| base.title.clone()),
        icon: over.icon.clone().or_else(|| base.icon.clone()),
        popup: over.popup.clone().or_else(|| base.popup.clone()),
        badge_text: over.badge_text.clone().or_else(|| base.badge_text.clone()),
        badge_background_color: over.badge_background_color.or(base.badge_background_color),
        badge_text_color: over.badge_text_color.or(base.badge_text_color),
        enabled: over.enabled.or(base.enabled),
        pattern_matching: over.pattern_matching.or(base.pattern_matching),
    }
}
