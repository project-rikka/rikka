use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use rikka_clipboard_core::{
    Error, Offer, Result, Selection, Session, URI_LIST, marks_secret, text_mime, uri_list,
};

use crate::watch::Change;
use crate::win32::owner::{Owner, Request};

const TRANSFER: Duration = Duration::from_secs(5);
const WHOLE_TRANSFER: usize = 256 * 1024 * 1024;

static PUBLISHERS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Clipboard {
    owner: Arc<Owner>,
    publisher: u64,
}

impl Clipboard {
    pub fn connect() -> Result<Clipboard> {
        Ok(Clipboard {
            owner: Arc::new(Owner::start()?),
            publisher: next_publisher(),
        })
    }

    #[must_use]
    pub fn share(&self) -> Clipboard {
        Clipboard {
            owner: Arc::clone(&self.owner),
            publisher: next_publisher(),
        }
    }

    pub fn close(&self) {
        self.owner.stop();
    }

    pub fn publish(&self, selection: Selection, offer: Offer) -> Result<()> {
        self.owner.send(Request::Publish {
            selection,
            offer,
            by: self.publisher,
        })
    }

    pub fn clear(&self, selection: Selection) -> Result<()> {
        self.publish(selection, Offer::new())
    }

    pub fn watch(&self, watcher: impl Fn(&Change) + Send + 'static) -> Result<()> {
        self.owner.send(Request::Watch {
            by: self.publisher,
            watcher: Box::new(watcher),
        })
    }

    pub fn mime_types(&self, selection: Selection) -> Result<Vec<String>> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.owner.send(Request::MimeTypes { selection, reply })?;
        receive(&answer)
    }

    pub fn read(&self, selection: Selection, mime: &str) -> Result<Vec<u8>> {
        self.read_within(selection, mime, WHOLE_TRANSFER)
    }

    pub fn read_within(&self, selection: Selection, mime: &str, bytes: usize) -> Result<Vec<u8>> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.owner.send(Request::Read {
            selection,
            mime: mime.to_owned(),
            limit: bytes,
            reply,
        })?;
        receive(&answer)
    }

    pub fn text(&self, selection: Selection) -> Result<String> {
        let offered = self.mime_types(selection)?;
        let mime = text_mime(&offered).ok_or(Error::Empty)?;
        String::from_utf8(self.read(selection, mime)?).map_err(|_| Error::NotUtf8)
    }

    #[must_use]
    pub fn is_secret(&self, change: &Change) -> bool {
        marks_secret(change.mime_types(), |hint| {
            self.read(change.selection(), hint).ok()
        })
    }

    pub fn set_text(&self, text: impl AsRef<str>) -> Result<()> {
        self.publish(Selection::Clipboard, Offer::text(text))
    }

    pub fn set_png(&self, png: impl Into<Arc<[u8]>>) -> Result<()> {
        self.publish(Selection::Clipboard, Offer::png(png))
    }

    pub fn uris(&self) -> Result<Vec<String>> {
        let bytes = self.read(Selection::Clipboard, URI_LIST)?;
        let text = String::from_utf8(bytes).map_err(|_| Error::NotUtf8)?;
        Ok(uri_list(&text))
    }
}

fn receive<T>(answer: &mpsc::Receiver<Result<T>>) -> Result<T> {
    match answer.recv_timeout(TRANSFER) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(Error::Unavailable("clipboard worker timed out"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(Error::Unavailable("clipboard worker stopped"))
        }
    }
}

fn next_publisher() -> u64 {
    PUBLISHERS.fetch_add(1, Ordering::Relaxed)
}

impl Session for Clipboard {
    fn mime_types(&self, selection: Selection) -> Result<Vec<String>> {
        Clipboard::mime_types(self, selection)
    }

    fn read(&self, selection: Selection, mime: &str) -> Result<Vec<u8>> {
        Clipboard::read(self, selection, mime)
    }

    fn publish(&self, selection: Selection, offer: Offer) -> Result<()> {
        Clipboard::publish(self, selection, offer)
    }

    fn watch(&self, changed: Box<dyn Fn() + Send>) -> Result<()> {
        Clipboard::watch(self, move |_| changed())
    }

    fn close(&self) {
        Clipboard::close(self);
    }
}
