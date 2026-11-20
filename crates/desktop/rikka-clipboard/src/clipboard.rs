use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rikka_wayland::{Display, Host};
use rikka_worker::StreamLimit;

use rikka_clipboard_core::{
    Error, Offer, Result, Selection, Session, URI_LIST, marks_secret, text_mime, uri_list,
};

use crate::error;
use crate::watch::Change;
use crate::worker::{self, Content, Link, Request, Worker};

const TRANSFER: Duration = Duration::from_secs(5);
const WHOLE_TRANSFER: StreamLimit = StreamLimit {
    time: Duration::from_secs(30),
    bytes: 256 * 1024 * 1024,
};

static PUBLISHERS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Clipboard {
    worker: Arc<Worker>,
    publisher: u64,
}

impl Clipboard {
    pub fn for_host(host: &Host) -> Result<Clipboard> {
        Ok(Clipboard::from_worker(worker::start(
            host.clone(),
            Link::Shared,
        )?))
    }

    pub fn connect() -> Result<Clipboard> {
        let display = Display::from_env().map_err(error::display)?;
        let host = Host::with_reader(display).map_err(error::display)?;
        Ok(Clipboard::from_worker(worker::start(host, Link::Owned)?))
    }

    #[must_use]
    pub fn share(&self) -> Clipboard {
        Clipboard {
            worker: Arc::clone(&self.worker),
            publisher: next_publisher(),
        }
    }

    fn from_worker(worker: Worker) -> Clipboard {
        Clipboard {
            worker: Arc::new(worker),
            publisher: next_publisher(),
        }
    }

    pub fn close(&self) {
        self.worker.stop();
    }

    pub fn publish(&self, selection: Selection, offer: Offer) -> Result<()> {
        self.send(Request::Publish {
            content: Content::Data(offer),
            selection,
            by: self.publisher,
        })
    }

    pub fn publish_deferred(
        &self,
        selection: Selection,
        mime_types: Vec<String>,
        deliver: impl Fn(&str, OwnedFd) + Send + 'static,
    ) -> Result<()> {
        self.send(Request::Publish {
            content: Content::Deferred {
                mime_types,
                deliver: Box::new(deliver),
            },
            selection,
            by: self.publisher,
        })
    }

    pub fn withdraw(&self, selection: Selection) -> Result<()> {
        self.send(Request::Withdraw {
            selection,
            by: self.publisher,
        })
    }

    pub fn clear(&self, selection: Selection) -> Result<()> {
        self.publish(selection, Offer::new())
    }

    pub fn watch(&self, watcher: impl Fn(&Change) + Send + 'static) -> Result<()> {
        self.send(Request::Watch {
            by: self.publisher,
            watcher: Box::new(watcher),
        })
    }

    pub fn mime_types(&self, selection: Selection) -> Result<Vec<String>> {
        self.worker
            .handle()
            .request(
                |reply| Request::MimeTypes { selection, reply }.into(),
                TRANSFER,
            )
            .map_err(error::stopped)
    }

    pub fn read(&self, selection: Selection, mime: &str) -> Result<Vec<u8>> {
        self.read_within(selection, mime, WHOLE_TRANSFER.bytes)
    }

    pub fn read_within(&self, selection: Selection, mime: &str, bytes: usize) -> Result<Vec<u8>> {
        let stream = self.worker.handle().request(
            |reply| {
                Request::Receive {
                    mime: mime.to_owned(),
                    selection,
                    reply,
                }
                .into()
            },
            TRANSFER,
        );
        let stream = stream.map_err(error::stopped)??;
        Ok(rikka_worker::read_stream(
            stream,
            StreamLimit {
                bytes,
                ..WHOLE_TRANSFER
            },
        )?)
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

    fn send(&self, request: Request) -> Result<()> {
        self.worker
            .handle()
            .send(request.into())
            .map_err(error::stopped)
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
