use rikka_seam::{Call, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext};
use shell_kit::{Host, Presence, PresenceService, Slot, Typist, copy_to_clipboard};

use crate::CAPABILITY;

pub(super) const INSERT: &str = "insert";

#[derive(Debug)]
pub(super) struct Inserting {
    pub(super) typed: bool,
}

#[derive(Debug)]
pub(super) struct EmojiService {
    presence: PresenceService,
    inserting: Slot<Inserting>,
}

impl EmojiService {
    #[must_use]
    pub(crate) fn new(inserting: Slot<Inserting>) -> EmojiService {
        EmojiService {
            presence: PresenceService::new(CAPABILITY, "Emoji picker", "the emoji picker"),
            inserting,
        }
    }

    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.presence.presence()
    }

    fn insert(&mut self, host: &Host, call: Call) {
        let text = match call.decode::<String>() {
            Ok(text) => text,
            Err(error) => return call.fail(error),
        };
        if text.is_empty() {
            return call.fail(SeamError::InvalidParams("empty emoji".to_owned()));
        }

        self.presence.hide();
        match host.get(self.inserting).typed {
            true => self.type_out(host.typist(), text),
            false => copy_to_clipboard(&text),
        }
        call.done();
    }

    fn type_out(&self, typist: &Typist, text: String) {
        if typist.text_keyboard().is_none() {
            return copy_to_clipboard(&text);
        }
        let typist = typist.clone();
        self.presence.presence().after_unmap(move || {
            let typed = typist
                .text_keyboard()
                .map(|keyboard| keyboard.type_text(&text));
            if let Some(Err(error)) = typed {
                tracing::warn!(%error, "failed to type emoji, copied it instead");
                copy_to_clipboard(&text);
            }
        });
    }
}

impl Service<Host> for EmojiService {
    fn manifest(&self) -> Manifest {
        self.presence.manifest().method(
            MethodInfo::new(INSERT)
                .summary("Types the emoji into the focused window, or copies it"),
        )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.presence.attach(seam);
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            INSERT => self.insert(host, call),
            _ => self.presence.answer(method, call),
        }
    }
}
