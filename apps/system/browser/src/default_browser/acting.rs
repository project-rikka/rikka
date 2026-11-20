use rikka_app::prelude::*;

use crate::default_browser::{Attempt, DefaultStatus, Failure, claim};
use crate::shell::Shell;

const CHANGES: &str = "rikka-browser-default-changes";
const CLAIMING: &str = "rikka-browser-default-claim";

impl Shell {
    pub(crate) fn attend_default_browser(&mut self) {
        let changes = self.browser().services().default_browser.changes();
        stream_keyed(
            CHANGES,
            changes,
            |shell: &mut Shell, status: DefaultStatus| {
                shell.default_status_changed(status);
            },
        );
        self.offer_default_browser();
    }

    pub(crate) fn make_default_browser(&mut self) {
        if self.claim.attempt() == Attempt::Making {
            return;
        }
        let Some(apps) = self.browser().services().default_browser.apps() else {
            return;
        };
        self.claim.set(Attempt::Making);
        spawn_blocking_keyed(
            CLAIMING,
            move || claim(&apps),
            |shell: &mut Shell, outcome: Result<DefaultStatus, Failure>| {
                shell.default_browser_claimed(outcome);
            },
        );
    }

    pub(crate) fn dismiss_default_prompt(&mut self) {
        self.browser().services().default_browser.finish_prompt();
        skip_view();
    }

    pub(crate) fn stop_asking_default(&mut self) {
        self.browser().preferences().set_check_default(false);
        self.dismiss_default_prompt();
    }

    fn default_status_changed(&mut self, status: DefaultStatus) {
        if status == DefaultStatus::Default {
            self.claim.forget_failure();
        }
        self.browser().services().default_browser.learned(status);
        self.offer_default_browser();
        skip_view();
    }

    fn default_browser_claimed(&mut self, outcome: Result<DefaultStatus, Failure>) {
        match outcome {
            Ok(status) => {
                self.claim.set(Attempt::Idle);
                self.browser().services().default_browser.learned(status);
            }
            Err(failure) => {
                tracing::warn!(?failure, "failed to set default browser");
                self.claim.set(Attempt::Failed(failure));
            }
        }
        skip_view();
    }

    fn offer_default_browser(&self) {
        if self.private() || self.little() {
            return;
        }
        let check_default = self.browser().preferences().check_default();
        self.browser()
            .services()
            .default_browser
            .offer(self.id(), check_default);
    }
}
