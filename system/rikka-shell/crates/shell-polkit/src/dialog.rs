use rikka_polkit::{Agent, Ask, Reply, Secret};
use rikka_seam::{Payload, Request, SeamClient, SeamError};
use rikka_session::following;
use rikka_ui::{State, TextDocument, spawn, stream};

use crate::service::{CAPABILITY, CHANGED, PENDING};

#[derive(Debug, State)]
pub(super) struct Dialog {
    seam: SeamClient,
    agent: Agent,
    asking: Option<Ask>,
    typed: TextDocument,
    details: bool,
}

fn answering(ask: &Ask) -> Option<u64> {
    ask.question.as_ref().map(|question| question.serial)
}

impl Dialog {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, agent: Agent) -> Dialog {
        Dialog {
            seam,
            agent,
            asking: None,
            typed: TextDocument::default(),
            details: false,
        }
    }

    pub(crate) fn start(&mut self) {
        let announced =
            following::states::<Option<Box<Ask>>>(&self.seam, CAPABILITY, CHANGED, || {
                Request::new(CAPABILITY, PENDING)
            });
        stream(
            announced,
            |dialog: &mut Dialog, asking: Option<Box<Ask>>| {
                dialog.show_ask(asking);
            },
        );
        spawn(
            self.seam.call(Request::new(CAPABILITY, PENDING)),
            |dialog: &mut Dialog, reply: Result<Payload, SeamError>| {
                let asking = match reply.and_then(|payload| payload.decode()) {
                    Ok(asking) => asking,
                    Err(error) => {
                        tracing::warn!("seam call {CAPABILITY}/{PENDING} failed: {error}");
                        None
                    }
                };
                dialog.show_ask(asking);
            },
        );
    }

    pub(crate) fn asking(&self) -> Option<&Ask> {
        self.asking.as_ref()
    }

    pub(crate) fn typed(&self) -> &TextDocument {
        &self.typed
    }

    pub(super) fn shows_details(&self) -> bool {
        self.details
    }

    pub(super) fn show_details(&mut self, shown: bool) {
        self.details = shown;
    }

    fn show_ask(&mut self, asking: Option<Box<Ask>>) {
        let Some(asking) = asking else {
            return;
        };
        let shown = self.asking.as_ref();
        if shown.is_none_or(|ask| ask.id != asking.id) {
            self.details = false;
        }
        if shown.and_then(answering) != answering(&asking) {
            self.typed = TextDocument::default();
        }
        self.asking = Some(*asking);
    }

    pub(crate) fn submit(&mut self) {
        let Some(ask) = &self.asking else {
            return;
        };
        let Some(question) = &ask.question else {
            return;
        };
        self.agent.answer(
            ask.id,
            Reply::Response {
                serial: question.serial,
                secret: Secret::new(self.typed.text()),
            },
        );
        self.typed = TextDocument::default();
    }

    pub(crate) fn answer(&self, reply: Reply) {
        if let Some(ask) = &self.asking {
            self.agent.answer(ask.id, reply);
        }
    }
}
