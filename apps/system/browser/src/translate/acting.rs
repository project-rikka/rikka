use browser_translators::{CodeMode, Display, Engine};
use rikka_languages::Language;

use crate::preferences::Rule;
use crate::shell::Shell;
use crate::translate::tabs::Choice;
use browser_model::TabId;

impl Shell {
    pub(crate) fn toggle_translation(&mut self) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let shown = self.browser().translations().tabs().shows_translation(tab);
        match shown {
            true => self.show_original(tab),
            false => self.start_translation(tab),
        }
    }

    pub(crate) fn start_translation(&mut self, tab: TabId) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        let preferences = self.browser().preferences();
        let to = preferences.translate_to();
        let choice = Choice {
            engine: preferences.translate_engine(),
            display: preferences.translate_display(),
            code: preferences.translate_code(),
        };
        let translations = self.browser().translations();
        translations.tabs_mut().attach(tab, &view);
        let command = translations.tabs_mut().start(tab, to, choice);
        if let Some(command) = command {
            command.send();
        }
    }

    pub(crate) fn show_original(&mut self, tab: TabId) {
        let command = self.browser().translations().tabs_mut().restore(tab);
        if let Some(command) = command {
            command.send();
        }
    }

    pub(crate) fn translate_into(&mut self, tab: TabId, target: Language) {
        self.browser()
            .preferences()
            .set_translate_target(Some(&target));
        self.translate_again(tab);
    }

    pub(crate) fn translate_with(&mut self, tab: TabId, engine: Engine) {
        self.browser().preferences().set_translate_engine(engine);
        self.translate_again(tab);
    }

    pub(crate) fn translate_code(&mut self, tab: TabId, code: CodeMode) {
        self.browser().preferences().set_translate_code(code);
        self.translate_again(tab);
    }

    pub(crate) fn translate_display(&mut self, tab: TabId, display: Display) {
        self.browser().preferences().set_translate_display(display);
        let command = self
            .browser()
            .translations()
            .tabs_mut()
            .redisplay(tab, display);
        if let Some(command) = command {
            command.send();
        }
    }

    pub(crate) fn translation_rule(&mut self, tab: TabId, rule: Rule, value: &str, on: bool) {
        self.browser()
            .preferences()
            .set_translation_rule(rule, value, on);
        let shown = self.browser().translations().tabs().shows_translation(tab);
        match (rule, on, shown) {
            (Rule::AlwaysLanguage | Rule::AlwaysSite, true, false) => self.start_translation(tab),
            (Rule::NeverLanguage | Rule::NeverSite, true, _) => {
                self.close_translate_popover();
                if shown {
                    self.show_original(tab);
                }
            }
            _ => {}
        }
    }

    fn translate_again(&mut self, tab: TabId) {
        if self.browser().translations().tabs().shows_translation(tab) {
            self.start_translation(tab);
        }
    }
}
