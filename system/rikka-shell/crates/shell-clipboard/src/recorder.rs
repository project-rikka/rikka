use std::sync::{Arc, mpsc};

use rikka_clipboard::{Change, Clipboard, Selection};

use crate::capture;
use crate::history::History;

pub(crate) fn start(clipboard: Arc<Clipboard>, history: History) -> rikka_clipboard::Result<()> {
    let (announcements, changes) = mpsc::channel::<Change>();
    clipboard.watch(move |change| {
        if change.selection() == Selection::Primary
            || change.is_ours()
            || !capture::should_record(change)
        {
            return;
        }
        if announcements.send(change.clone()).is_err() {
            tracing::debug!("clipboard history recorder stopped");
        }
    })?;

    std::thread::Builder::new()
        .name("rikka-clipboard-history".into())
        .spawn(move || {
            for change in changes {
                if let Some(content) = capture::capture(&clipboard, &change) {
                    history.record(content);
                }
            }
        })
        .map_err(rikka_clipboard::Error::Io)?;
    Ok(())
}
