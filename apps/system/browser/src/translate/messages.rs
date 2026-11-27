use browser_translators::{Job, Message, PageReport, Request, Watch, detect};
use rikka_app::prelude::*;
use rikka_languages::Language;
use rikka_web::ScriptAsk;
use serde_json::Value;

use crate::shell::Shell;
use crate::translate::decide;
use crate::translate::failure::TranslateFailure;
use browser_model::TabId;

impl Shell {
    pub(crate) fn translation_message(&mut self, tab: TabId, body: &Value) {
        match Message::parse(body) {
            Ok(Message::Page(report)) => {
                if self.attach_translation(tab) {
                    self.page_reported(tab, &report);
                }
            }
            Ok(Message::Watch(_) | Message::Translate(_)) => {
                tracing::debug!("translation request posted without a reply");
            }
            Err(error) => tracing::debug!(%error, "invalid translation message"),
        }
    }

    pub(crate) fn translation_ask(&mut self, tab: TabId, ask: ScriptAsk) {
        let message = match Message::parse(&ask.body) {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%error, "invalid translation message");
                return;
            }
        };
        if !self.attach_translation(tab) {
            return;
        }
        match message {
            Message::Page(report) => {
                ask.respond(Value::Null);
                self.page_reported(tab, &report);
            }
            Message::Watch(watch) => self.frame_watches(tab, &watch, ask),
            Message::Translate(request) => self.translation_asked(tab, request, ask),
        }
    }

    fn attach_translation(&mut self, tab: TabId) -> bool {
        let Some(view) = self.view_of(tab) else {
            return false;
        };
        self.browser().translations().tabs_mut().attach(tab, &view);
        true
    }

    fn page_reported(&mut self, tab: TabId, report: &PageReport) {
        let detected = detect(report);
        let translations = self.browser().translations();
        translations.tabs_mut().detected(tab, detected);
        if translations.tabs().session(tab).is_some() {
            return;
        }
        let Some(address) = self.model().tab(tab).map(|tab_entry| tab_entry.address()) else {
            return;
        };
        let automatic = decide::decide(
            self.browser().preferences(),
            detected,
            &address,
            |page, rules| decide::automatic(page, rules, translations.tabs().follow(tab)),
        );
        if automatic {
            self.start_translation(tab);
        }
    }

    fn frame_watches(&mut self, tab: TabId, watch: &Watch, ask: ScriptAsk) {
        let translations = self.browser().translations();
        let answer = decide::answer(translations.tabs().session(tab), watch);
        match answer {
            Some(command) => ask.respond(command.answer()),
            None => drop(translations.tabs_mut().hold(tab, *watch, ask)),
        }
    }

    fn translation_asked(&mut self, tab: TabId, request: Request, ask: ScriptAsk) {
        let translations = self.browser().translations();
        let Some(session) = translations.tabs().current(tab, request.session) else {
            return;
        };
        let translator = match translations.translator() {
            Ok(translator) => translator,
            Err(failure) => {
                translations.tabs_mut().failed(tab, session.id, failure);
                return;
            }
        };
        translations.tabs_mut().sent(tab, session.id);
        let engine = session.engine;
        let job = Job::new(session.to, request.units)
            .protect(self.browser().preferences().translate_protect());
        let id = session.id;
        spawn(
            async move {
                match translator.translate(engine, job).await {
                    Ok(translation) => {
                        ask.respond(translation.to_json());
                        Ok(translation.detected)
                    }
                    Err(error) => {
                        tracing::debug!(%error, "translation batch failed");
                        Err(TranslateFailure::from_error(&error))
                    }
                }
            },
            move |shell: &mut Shell, outcome: Result<Option<Language>, TranslateFailure>| {
                shell
                    .browser()
                    .translations()
                    .tabs_mut()
                    .batch_finished(tab, id, outcome);
            },
        );
    }
}
