use rikka_image_edit::Recipe;

use super::Paint;
use crate::direct::Direct;
use crate::session::LastEffect;
use crate::tab::Tab;

impl Paint {
    pub(crate) fn develops(&self) -> bool {
        self.edits()
            && self
                .active_tab()
                .is_some_and(|tab| tab.develop_refusal().is_none())
    }

    pub(crate) fn last_effect(&self) -> Option<&LastEffect> {
        self.last_effect.as_ref()
    }

    pub(crate) fn develop_directly(&mut self, direct: Direct) {
        let recipe = direct.recipe();
        if self.develop(&recipe) && direct.is_effect() {
            self.last_effect = Some(LastEffect::new(direct.name(), recipe, None));
        }
    }

    pub(crate) fn repeat_effect(&mut self) {
        if let Some(recipe) = self.last_effect.as_ref().map(|last| last.recipe().clone()) {
            self.develop(&recipe);
        }
    }

    pub(crate) fn reopen_effect(&mut self) {
        let Some(last) = self.last_effect.clone() else {
            return;
        };
        let session = self
            .active_tab()
            .and_then(Tab::context)
            .and_then(|context| last.reopen(&context));
        match session {
            Some(session) => self.edit(|tab| tab.begin_session(session)),
            None => {
                self.develop(last.recipe());
            }
        }
    }

    fn develop(&mut self, recipe: &Recipe) -> bool {
        if !self.edits() {
            return false;
        }
        let Some(tab) = self.active_tab_mut() else {
            return false;
        };
        match tab.develop(recipe) {
            Ok(()) => true,
            Err(refusal) => {
                self.notice = Some(refusal);
                false
            }
        }
    }
}
