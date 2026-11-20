use crate::prelude::*;
use rikka_llm_accounts::{Backend, Prompt, Step, sign_in};

use crate::llm_settings::LlmSettings;

#[derive(Debug)]
pub(crate) struct Signing {
    backend: Backend,
    nth: u64,
    prompt: Option<Prompt>,
    cancel: CancelToken,
}

impl Signing {
    pub(crate) fn start(backend: Backend, nth: u64) -> Signing {
        let cancel = CancelToken::new();
        stream(
            sign_in(backend, cancel.clone()),
            move |settings: &mut LlmSettings, step: Step| {
                settings.accounts.sign_in_step(nth, step, &mut settings.llm)
            },
        );
        Signing {
            backend,
            nth,
            prompt: None,
            cancel,
        }
    }

    pub(crate) fn backend(&self) -> Backend {
        self.backend
    }

    pub(crate) fn is(&self, nth: u64) -> bool {
        self.nth == nth
    }

    pub(crate) fn prompt(&self) -> Option<&Prompt> {
        self.prompt.as_ref()
    }

    pub(crate) fn prompted(&mut self, prompt: Prompt) {
        self.prompt = Some(prompt);
    }

    pub(crate) fn stop(&self) {
        self.cancel.cancel();
    }
}
