use std::sync::Arc;

use rikka_clipboard::{Clipboard, Selection};
use rikka_input::Chord;
use rikka_seam::{Call, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext};
use rikka_types::{Key, Modifiers};
pub(crate) use shell_kit::{CLOSE, TOGGLE};
use shell_kit::{Host, Presence, PresenceService, Slot, Typist};

use crate::history::History;
use crate::recorder;

pub(crate) const CAPABILITY: &str = "rikka.clipboard";

pub(super) const PASTE: &str = "paste";

pub(super) const COPY: &str = "copy";

fn paste_chord() -> Chord {
    Chord::new(Modifiers::CONTROL, Key::Char('v'))
}

#[derive(Debug)]
pub(super) struct Pasting {
    pub(super) automatic: bool,
}

pub(super) struct ClipboardService {
    history: History,
    clipboard: Option<Arc<Clipboard>>,
    pasting: Slot<Pasting>,
    presence: PresenceService,
}

impl ClipboardService {
    #[must_use]
    pub(crate) fn new(
        clipboard: Option<Clipboard>,
        history: History,
        pasting: Slot<Pasting>,
    ) -> ClipboardService {
        let clipboard = clipboard.map(Arc::new);
        if let Some(clipboard) = &clipboard
            && let Err(error) = recorder::start(clipboard.clone(), history.clone())
        {
            tracing::warn!(%error, "cannot record clipboard history");
        }
        ClipboardService {
            history,
            clipboard,
            pasting,
            presence: PresenceService::new(
                CAPABILITY,
                "Clipboard history",
                "the clipboard history",
            ),
        }
    }

    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.presence.presence()
    }

    fn restore(&mut self, host: &Host, call: Call, paste: bool) {
        let id = match call.decode::<u64>() {
            Ok(id) => id,
            Err(error) => return call.fail(error),
        };
        let Some(entry) = self.history.get(id) else {
            return call.fail(SeamError::InvalidParams(format!("no clipboard entry {id}")));
        };
        let Some(clipboard) = &self.clipboard else {
            return call.fail(SeamError::Unavailable(
                "this session has no clipboard".to_owned(),
            ));
        };
        if let Err(error) = clipboard.publish(Selection::Clipboard, entry.content.offer()) {
            return call.fail(SeamError::Unavailable(error.to_string()));
        }

        self.presence.hide();
        if paste && host.get(self.pasting).automatic {
            self.press_paste(host.typist());
        }
        call.done();
    }

    fn press_paste(&self, typist: &Typist) {
        if typist.text_keyboard().is_none() {
            return;
        }
        let typist = typist.clone();
        self.presence.presence().after_unmap(move || {
            let pressed = typist
                .text_keyboard()
                .map(|keyboard| keyboard.press(paste_chord()));
            if let Some(Err(error)) = pressed {
                tracing::warn!(%error, "failed to paste clipboard entry");
            }
        });
    }
}

impl Service<Host> for ClipboardService {
    fn manifest(&self) -> Manifest {
        self.presence
            .manifest()
            .method(
                MethodInfo::new(PASTE)
                    .summary("Copies history entry `id` to the clipboard and pastes it"),
            )
            .method(MethodInfo::new(COPY).summary("Copies history entry `id` to the clipboard"))
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.presence.attach(seam);
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            PASTE => self.restore(host, call, true),
            COPY => self.restore(host, call, false),
            _ => self.presence.answer(method, call),
        }
    }
}

impl std::fmt::Debug for ClipboardService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardService")
            .field("history", &self.history)
            .field("presence", &self.presence)
            .field("pasting", &self.pasting)
            .finish_non_exhaustive()
    }
}
