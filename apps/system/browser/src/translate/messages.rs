use browser_translators::{Job, Message, PageReport, Request, Watch, detect};
use rikka_app::prelude::*;
use rikka_languages::Language;
use rikka_web::{ScriptMessage, ScriptReply};
use serde_json::Value;

use crate::shell::Shell;
use crate::translate::decide;
use crate::translate::failure::TranslateFailure;
use browser_model::TabId;

const STALE: &str = "stale translation session";
const UNAVAILABLE: &str = "translator unavailable";

impl Shell {
    pub(crate) fn handle_translation_message(&mut self, tab: TabId, message: ScriptMessage) {
        let ScriptMessage { body, reply, .. } = message;
        let Some(reply) = reply else {
            return;
        };
        let message = match Message::parse(&body) {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%error, "invalid translation message");
                reply.fail(error.to_string());
                return;
            }
        };
        let Some(view) = self.view_of(tab) else {
            reply.fail(STALE);
            return;
        };
        self.browser().translations().tabs_mut().attach(tab, &view);
        match message {
            Message::Page(report) => self.page_reported(tab, &report, reply),
            Message::Watch(watch) => self.frame_watches(tab, &watch, reply),
            Message::Translate(request) => self.translation_asked(tab, request, reply),
        }
    }

    fn page_reported(&mut self, tab: TabId, report: &PageReport, reply: ScriptReply) {
        reply.send(Value::Null);
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

    fn frame_watches(&mut self, tab: TabId, watch: &Watch, reply: ScriptReply) {
        let translations = self.browser().translations();
        let answer = decide::answer(translations.tabs().session(tab), watch);
        match answer {
            Some(command) => reply.send(command.answer(watch)),
            None => {
                let evicted = translations.tabs_mut().hold(tab, *watch, reply);
                if let Some(evicted) = evicted {
                    evicted.send(Value::Null);
                }
            }
        }
    }

    fn translation_asked(&mut self, tab: TabId, request: Request, reply: ScriptReply) {
        let translations = self.browser().translations();
        let current = translations.tabs().current(tab, request.session);
        let Some(session) = current else {
            reply.fail(STALE);
            return;
        };
        let translator = match translations.translator() {
            Ok(translator) => translator,
            Err(failure) => {
                reply.fail(UNAVAILABLE);
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
                        reply.send(translation.to_json());
                        Ok(translation.detected)
                    }
                    Err(error) => {
                        tracing::debug!(%error, "translation batch failed");
                        reply.fail(error.to_string());
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
