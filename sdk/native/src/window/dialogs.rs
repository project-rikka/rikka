use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use rikka_ui::{Anchor, Element, Handler, View as _, fade};
use rikka_widgets::{alert_prompt, confirm_prompt, scrim};

use crate::ffi;
use crate::handlers::Callback;
use crate::view::STAGE;
use crate::window::Model;

const KEY: &str = "rikka-sdk-dialog";

enum Prompt {
    Alert,
    Confirm {
        accept: String,
        answer: Callback<ffi::BoolFn>,
    },
}

struct Dialog {
    serial: u64,
    title: String,
    message: String,
    prompt: Prompt,
}

#[derive(Default)]
pub(crate) struct Dialogs {
    queue: RefCell<VecDeque<Dialog>>,
    next: Cell<u64>,
}

impl Dialogs {
    fn push(&self, title: &str, message: &str, prompt: Prompt) {
        let serial = self.next.get();
        self.next.set(serial + 1);
        self.queue.borrow_mut().push_back(Dialog {
            serial,
            title: title.to_owned(),
            message: message.to_owned(),
            prompt,
        });
    }

    pub(crate) fn alert(&self, title: &str, message: &str) {
        self.push(title, message, Prompt::Alert);
    }

    pub(crate) fn confirm(
        &self,
        title: &str,
        message: &str,
        accept: &str,
        answer: Callback<ffi::BoolFn>,
    ) {
        let prompt = Prompt::Confirm {
            accept: accept.to_owned(),
            answer,
        };
        self.push(title, message, prompt);
    }

    fn take(&self, serial: u64) -> Option<Dialog> {
        let mut queue = self.queue.borrow_mut();
        match queue.front() {
            Some(front) if front.serial == serial => queue.pop_front(),
            _ => None,
        }
    }

    pub(crate) fn elements(&self) -> Vec<Element> {
        let queue = self.queue.borrow();
        let Some(dialog) = queue.front() else {
            return Vec::new();
        };
        let serial = dialog.serial;
        let closing = || Handler::new(move |model: &mut Model| model.answer(serial, false));
        let panel = match &dialog.prompt {
            Prompt::Alert => alert_prompt(dialog.title.clone(), dialog.message.clone(), closing()),
            Prompt::Confirm { accept, .. } => confirm_prompt(
                dialog.title.clone(),
                dialog.message.clone(),
                accept.clone(),
                Handler::new(move |model: &mut Model, accepted: bool| {
                    model.answer(serial, accepted);
                }),
            ),
        };
        vec![
            scrim()
                .on_dismiss(closing())
                .transition(fade())
                .anchored(Anchor::covering(STAGE))
                .keyed((KEY, serial, STAGE)),
            panel.keyed((KEY, serial)).centered_on(STAGE).build(),
        ]
    }
}

impl Model {
    fn answer(&mut self, serial: u64, accepted: bool) {
        let Some(dialog) = self.dialogs.take(serial) else {
            return;
        };
        if let Prompt::Confirm { answer, .. } = dialog.prompt {
            answer.call(&mut self.context(), accepted);
        }
    }
}

impl std::fmt::Debug for Dialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dialogs")
            .field("queued", &self.queue.borrow().len())
            .finish()
    }
}
