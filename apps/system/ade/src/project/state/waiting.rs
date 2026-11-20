use rikka_app::prelude::*;
use rikka_chat::Draft;
use rikka_thread::Ask;

use crate::setup::Setup;

use super::Project;
use super::conversing::StartError;

impl Project {
    pub(super) fn interject(&mut self, ask: Ask) -> Result<(), StartError> {
        let Some(runner) = self.runner_mut() else {
            return Err(StartError::Silent);
        };
        runner
            .interject(ask)
            .map(|_| ())
            .map_err(|_| StartError::Silent)
    }

    pub(in crate::project) fn recall(&mut self, id: u64) {
        let Some(ask) = self.runner_mut().and_then(|runner| runner.unqueue(id)) else {
            return;
        };
        self.redraft(vec![ask]);
    }

    pub(in crate::project) fn withdraw(&mut self, id: u64) {
        if let Some(runner) = self.runner_mut() {
            let _withdrawn = runner.unqueue(id);
        }
    }

    pub(super) fn redraft(&mut self, asks: Vec<Ask>) {
        if asks.is_empty() {
            return;
        }
        let typed = Ask::new(self.draft.text());
        let Some((text, attached)) =
            Ask::join(asks.into_iter().chain([typed])).map(Ask::into_draft)
        else {
            return;
        };
        self.draft.set_text(text);
        self.attached.splice(0..0, attached);
    }

    pub(super) fn unheard(&mut self, thread: &str, carried_on: bool) {
        let Some(runner) = self.runners.get_mut(thread) else {
            return;
        };
        let unheard = runner.unheard();
        if unheard.is_empty() {
            return;
        }
        if !carried_on {
            self.redraft_in(thread, unheard);
            return;
        }
        let thread = thread.to_owned();
        self.handle
            .say_with_setup(move |project, setup| project.followed(&thread, unheard, setup));
    }

    fn followed(&mut self, thread: &str, asks: Vec<Ask>, setup: &Setup) {
        let Some(ask) = Ask::join(asks) else {
            return;
        };
        let Some(runner) = self.runners.get_mut(thread) else {
            self.redraft_in(thread, vec![ask]);
            return;
        };
        if runner.is_running() {
            if let Err(ask) = runner.interject(ask) {
                self.redraft_in(thread, vec![ask]);
            }
            return;
        }

        let asked = ask.clone();
        let begun = self.start_in(thread, setup, move |runner, hired| {
            runner.ask(hired, asked).map(|()| true)
        });
        match begun {
            Ok(true) => {}
            Ok(false) => self.redraft_in(thread, vec![ask]),
            Err(text) => {
                self.redraft_in(thread, vec![ask]);
                self.report_error(text, Some(thread.to_owned()));
            }
        }
    }

    pub(super) fn redraft_in(&mut self, thread: &str, asks: Vec<Ask>) {
        if *self.open == thread {
            self.redraft(asks);
            return;
        }
        if asks.is_empty() {
            return;
        }
        let draft = self.drafts.restore(thread).unwrap_or_default();
        let typed = Ask::new(draft.text.text()).with_attachments(draft.attached);
        let (text, attached) = Ask::join(asks.into_iter().chain([typed]))
            .map(Ask::into_draft)
            .unwrap_or_default();
        self.drafts
            .stash(thread, Draft::new(TextDocument::new(text), attached));
    }
}
