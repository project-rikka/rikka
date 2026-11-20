use std::convert::Infallible;
use std::sync::Arc;

use rikka_clipboard::{Change, Clipboard, Offer, Selection};
use rikka_remote_wire::LONGEST_CLIPBOARD;
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};
use tokio::sync::mpsc::UnboundedSender;

const THREAD: &str = "rikka-remote-viewer-clipboard";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ClipboardError {
    #[error("{0}")]
    Clipboard(#[from] rikka_clipboard::Error),
    #[error("cannot follow the clipboard: {0}")]
    Thread(#[from] std::io::Error),
    #[error("the clipboard reader stopped before it began")]
    Exited,
}

impl From<rikka_worker::Error<Infallible>> for ClipboardError {
    fn from(error: rikka_worker::Error<Infallible>) -> ClipboardError {
        match error {
            rikka_worker::Error::Spawn(error) => ClipboardError::Thread(error),
            rikka_worker::Error::Startup(never) => match never {},
            rikka_worker::Error::Exited => ClipboardError::Exited,
        }
    }
}

enum Reading {
    Changed(Change),
    Stop,
}

pub(crate) struct ClipboardSync {
    clipboard: Arc<Clipboard>,
    _reader: Worker<Arc<CommandSender<Reading>>>,
}

impl ClipboardSync {
    pub(crate) fn start(changes: UnboundedSender<String>) -> Result<ClipboardSync, ClipboardError> {
        let clipboard = Arc::new(Clipboard::connect()?);
        let (seen, readings) = command_channel(|| Reading::Stop)?;
        let seen = Arc::new(seen);
        let reader = Arc::clone(&clipboard);
        let worker = Worker::spawn(THREAD, Arc::clone(&seen), move || {
            Ok::<_, Infallible>(move || read(&reader, &readings, &changes))
        })?;
        clipboard.watch(move |change| {
            if change.selection() != Selection::Clipboard || change.is_ours() {
                return;
            }
            if let Err(error) = seen.send(Reading::Changed(change.clone())) {
                tracing::debug!("the clipboard reader stopped: {error}");
            }
        })?;
        Ok(ClipboardSync {
            clipboard,
            _reader: worker,
        })
    }

    pub(crate) fn set(&self, text: String) {
        if let Err(error) = self
            .clipboard
            .publish(Selection::Clipboard, Offer::text(text))
        {
            tracing::warn!("cannot set the clipboard: {error}");
        }
    }
}

impl Drop for ClipboardSync {
    fn drop(&mut self) {
        self.clipboard.close();
    }
}

fn read(
    clipboard: &Clipboard,
    readings: &CommandReceiver<Reading>,
    changes: &UnboundedSender<String>,
) {
    loop {
        for reading in readings.drain() {
            let Reading::Changed(change) = reading else {
                return;
            };
            if clipboard.is_secret(&change) {
                continue;
            }
            match clipboard.text(Selection::Clipboard) {
                Ok(text) if text.len() <= LONGEST_CLIPBOARD => {
                    if changes.send(text).is_err() {
                        return;
                    }
                }
                Ok(_) => tracing::debug!("the clipboard text is too long to send"),
                Err(error) => tracing::debug!("the clipboard holds no text: {error}"),
            }
        }
        if let Err(error) = readings.wait() {
            tracing::warn!("the clipboard readings stopped: {error}");
            return;
        }
    }
}
