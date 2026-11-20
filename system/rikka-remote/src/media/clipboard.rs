use std::sync::Arc;

use rikka_clipboard::{Change, Clipboard, Offer, Selection};
use rikka_remote_wire::{CLIPBOARD_TEXT, LONGEST_CLIPBOARD, ServerMessage};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};

use crate::media::{MediaError, MediaEvent, MediaSink};

const THREAD: &str = "rikka-remote-clipboard";

enum Reading {
    Changed(Change),
    Stop,
}

pub(crate) struct ClipboardSync {
    clipboard: Arc<Clipboard>,
    _reader: Worker<Arc<CommandSender<Reading>>>,
}

impl ClipboardSync {
    pub(crate) fn start(sink: MediaSink) -> Result<ClipboardSync, MediaError> {
        let clipboard = Arc::new(Clipboard::connect().map_err(MediaError::Clipboard)?);
        let (changes, received) =
            command_channel(|| Reading::Stop).map_err(|error| MediaError::Thread {
                thread: THREAD,
                error,
            })?;
        let changes = Arc::new(changes);
        let reader = Arc::clone(&clipboard);
        let worker = Worker::spawn(THREAD, Arc::clone(&changes), move || {
            Ok(move || read(&reader, &received, &sink))
        })
        .map_err(|error| MediaError::worker(THREAD, error))?;
        clipboard
            .watch(move |change| {
                if change.selection() != Selection::Clipboard || change.is_ours() {
                    return;
                }
                if let Err(error) = changes.send(Reading::Changed(change.clone())) {
                    tracing::debug!("clipboard reader stopped: {error}");
                }
            })
            .map_err(MediaError::Clipboard)?;
        Ok(ClipboardSync {
            clipboard,
            _reader: worker,
        })
    }

    pub(crate) fn set(&self, text: String) {
        if text.len() > LONGEST_CLIPBOARD {
            return;
        }
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

fn read(clipboard: &Clipboard, readings: &CommandReceiver<Reading>, sink: &MediaSink) {
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
                    let message = ServerMessage::Clipboard {
                        mime: CLIPBOARD_TEXT.to_owned(),
                        data: text.into_bytes(),
                    };
                    if sink.send(MediaEvent::Message(message)).is_err() {
                        return;
                    }
                }
                Ok(text) => tracing::debug!(bytes = text.len(), "clipboard text too long to send"),
                Err(error) => tracing::debug!("clipboard holds no text: {error}"),
            }
        }
        if let Err(error) = readings.wait() {
            tracing::warn!("clipboard readings stopped: {error}");
            return;
        }
    }
}
