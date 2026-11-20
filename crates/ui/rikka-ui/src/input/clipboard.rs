use rikka_sync::{Mutex, MutexGuard};
use std::sync::{Arc, OnceLock};

use rikka_clipboard_core::{Error, Offer, Result, Session};

use crate::task::{IntoHandler, spawn_blocking};

pub use rikka_clipboard_core::Selection;

type Watcher = Arc<dyn Fn() + Send + Sync>;

static SESSION: OnceLock<Box<dyn Session>> = OnceLock::new();
static HELD: Mutex<SessionClipboard> = Mutex::new(SessionClipboard {
    clipboard: Offer::new(),
    primary: Offer::new(),
    watchers: Vec::new(),
});

struct SessionClipboard {
    clipboard: Offer,
    primary: Offer,
    watchers: Vec<Watcher>,
}

impl SessionClipboard {
    fn offer(&mut self, selection: Selection) -> &mut Offer {
        match selection {
            Selection::Clipboard => &mut self.clipboard,
            Selection::Primary => &mut self.primary,
        }
    }
}

pub fn install_session(session: impl Session + 'static) {
    if SESSION.set(Box::new(session)).is_err() {
        tracing::debug!("session clipboard already installed");
    }
}

pub fn close_session() {
    if let Some(session) = SESSION.get() {
        session.close();
    }
}

#[must_use]
pub fn mime_types() -> Vec<String> {
    offered(Selection::Clipboard)
}

pub fn read(mime: &str) -> Result<Vec<u8>> {
    read_from(Selection::Clipboard, mime)
}

pub fn publish(offer: Offer) -> Result<()> {
    publish_to(Selection::Clipboard, offer)
}

pub fn watch(changed: impl Fn() + Send + Sync + 'static) -> Result<()> {
    match SESSION.get() {
        Some(session) => session.watch(Box::new(changed)),
        None => {
            state().watchers.push(Arc::new(changed));
            Ok(())
        }
    }
}

#[must_use]
pub fn text_of(selection: Selection) -> Option<String> {
    let offered = offered(selection);
    let mime = rikka_clipboard_core::text_mime(&offered)?;
    let text = String::from_utf8(read_from(selection, mime).ok()?).ok()?;
    Some(text).filter(|text| !text.is_empty())
}

pub fn set_text_of(selection: Selection, text: &str) {
    let offer = match text.is_empty() {
        true => Offer::new(),
        false => Offer::text(text),
    };
    if let Err(error) = publish_to(selection, offer) {
        tracing::warn!("failed to set clipboard text: {error}");
    }
}

#[must_use]
pub fn has_text() -> bool {
    rikka_clipboard_core::text_mime(&mime_types()).is_some()
}

#[must_use]
pub fn has_files() -> bool {
    rikka_clipboard_core::offers_files(&mime_types())
}

#[must_use]
pub fn files() -> Option<rikka_clipboard_core::Files> {
    rikka_clipboard_core::files_in(&mime_types(), |mime| read(mime).ok())
}

#[must_use]
pub fn text() -> Option<String> {
    text_of(Selection::Clipboard)
}

pub fn read_text_of<M>(selection: Selection, on_text: impl IntoHandler<(Option<String>,), (), M>) {
    spawn_blocking(move || text_of(selection), on_text);
}

pub fn read_text<M>(on_text: impl IntoHandler<(Option<String>,), (), M>) {
    read_text_of(Selection::Clipboard, on_text);
}

pub fn set_text(text: &str) {
    set_text_of(Selection::Clipboard, text);
}

fn offered(selection: Selection) -> Vec<String> {
    match SESSION.get() {
        Some(session) => session.mime_types(selection).unwrap_or_default(),
        None => state()
            .offer(selection)
            .mime_types()
            .map(str::to_owned)
            .collect(),
    }
}

fn read_from(selection: Selection, mime: &str) -> Result<Vec<u8>> {
    match SESSION.get() {
        Some(session) => session.read(selection, mime),
        None => state()
            .offer(selection)
            .data(mime)
            .map(|data| data.to_vec())
            .ok_or_else(|| Error::NoSuchType(mime.to_owned())),
    }
}

fn publish_to(selection: Selection, offer: Offer) -> Result<()> {
    if let Some(session) = SESSION.get() {
        return session.publish(selection, offer);
    }
    let watchers = {
        let mut shared = state();
        *shared.offer(selection) = offer;
        shared.watchers.clone()
    };
    for watcher in watchers {
        watcher();
    }
    Ok(())
}

fn state() -> MutexGuard<'static, SessionClipboard> {
    HELD.lock()
}
