use std::fs::File;
use std::io::Write;
use std::os::fd::OwnedFd;

use rikka_clipboard::{Clipboard, Selection};
use rikka_compositor_selection::{
    SelectionEvent, SelectionKind, SendSelection, Source, clear_selection, request_data,
    set_compositor_selection,
};

use super::state::State;
use crate::command::Command;

#[derive(Debug, Default)]
pub(crate) struct Bridge {
    clipboard: Option<Clipboard>,
    generations: [u64; 2],
}

fn selection_of(kind: SelectionKind) -> Selection {
    match kind {
        SelectionKind::Clipboard => Selection::Clipboard,
        SelectionKind::Primary => Selection::Primary,
    }
}

fn kind_of(selection: Selection) -> SelectionKind {
    match selection {
        Selection::Clipboard => SelectionKind::Clipboard,
        Selection::Primary => SelectionKind::Primary,
    }
}

fn slot(kind: SelectionKind) -> usize {
    match kind {
        SelectionKind::Clipboard => 0,
        SelectionKind::Primary => 1,
    }
}

fn withdraw(clipboard: &Clipboard, kind: SelectionKind) {
    if let Err(error) = clipboard.withdraw(selection_of(kind)) {
        tracing::warn!("failed to withdraw nested {kind} from the session: {error}");
    }
}

impl Bridge {
    pub(crate) fn disconnect(&mut self) {
        if let Some(clipboard) = self.clipboard.take() {
            for kind in SelectionKind::ALL {
                withdraw(&clipboard, kind);
            }
        }
    }
}

impl State {
    pub(crate) fn connect_session_clipboard(&mut self, clipboard: &Clipboard) {
        let clipboard = clipboard.share();
        let link = self.link.clone();
        let watched = clipboard.watch(move |change| {
            if change.is_ours() {
                return;
            }
            let command = Command::SessionSelection {
                kind: kind_of(change.selection()),
                mime_types: change.mime_types().to_vec(),
            };
            if link.send(command).is_err() {
                tracing::debug!("nested display stopped before a session selection change");
            }
        });
        if let Err(error) = watched {
            tracing::warn!("failed to watch session clipboard: {error}");
            return;
        }
        for kind in SelectionKind::ALL {
            match clipboard.mime_types(selection_of(kind)) {
                Ok(mime_types) => self.session_selection(kind, mime_types),
                Err(error) => tracing::warn!("failed to read session {kind}: {error}"),
            }
        }
        self.bridge.clipboard = Some(clipboard);
    }

    pub(crate) fn session_selection(&mut self, kind: SelectionKind, mime_types: Vec<String>) {
        let seat = self.seat;
        if mime_types.is_empty() {
            clear_selection(self, seat, kind);
            return;
        }
        let mimes = mime_types.into_iter().map(String::into_boxed_str).collect();
        if let Err(error) = set_compositor_selection(self, seat, kind, mimes) {
            tracing::warn!("failed to offer session {kind} to nested clients: {error}");
        }
    }

    pub(crate) fn send_session_selection(&mut self, request: SendSelection) {
        let Some(clipboard) = self.bridge.clipboard.as_ref().map(Clipboard::share) else {
            return;
        };
        let selection = selection_of(request.kind);
        let spawned = std::thread::Builder::new()
            .name("rikka-nested-paste".into())
            .spawn(move || {
                let data = match clipboard.read(selection, &request.mime_type) {
                    Ok(data) => data,
                    Err(error) => {
                        tracing::debug!("failed to read session selection: {error}");
                        return;
                    }
                };
                let mut file = File::from(request.fd);
                if let Err(error) = file.write_all(&data) {
                    tracing::debug!("nested client closed paste pipe: {error}");
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("failed to start paste: {error}");
        }
    }

    pub(crate) fn selection_event_arrived(&mut self, event: SelectionEvent) {
        match event {
            SelectionEvent::Changed { seat, kind } if seat == self.seat => {
                self.client_selection_changed(kind)
            }
            other => self.drag_event_arrived(other),
        }
    }

    fn client_selection_changed(&mut self, kind: SelectionKind) {
        let index = slot(kind);
        self.bridge.generations[index] += 1;
        let generation = self.bridge.generations[index];
        let Some(clipboard) = self.bridge.clipboard.as_ref() else {
            return;
        };
        let Some(current) = self.selections.current(self.seat, kind) else {
            withdraw(clipboard, kind);
            return;
        };
        if !matches!(current.source, Source::Client(_)) {
            return;
        }
        let mime_types = current
            .mime_types
            .iter()
            .map(|mime| mime.to_string())
            .collect();
        let link = self.link.clone();
        let published =
            clipboard.publish_deferred(selection_of(kind), mime_types, move |mime, fd| {
                let command = Command::ClientSend {
                    kind,
                    generation,
                    mime_type: mime.to_owned(),
                    fd,
                };
                if link.send(command).is_err() {
                    tracing::debug!("nested display stopped before a session paste");
                }
            });
        if let Err(error) = published {
            tracing::warn!("failed to forward nested {kind} to the session: {error}");
        }
    }

    pub(crate) fn client_send(
        &mut self,
        kind: SelectionKind,
        generation: u64,
        mime_type: String,
        fd: OwnedFd,
    ) {
        if self.bridge.generations[slot(kind)] != generation {
            return;
        }
        let seat = self.seat;
        if let Err(error) = request_data(self, seat, kind, mime_type, fd) {
            tracing::debug!("failed to read nested {kind} for the session: {error}");
        }
    }
}
