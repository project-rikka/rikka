use std::io;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use rikka_gecko_protocol::{EmbedderMessage, write_frame};

pub(crate) fn spawn(
    mut stream: UnixStream,
) -> io::Result<(Sender<EmbedderMessage>, JoinHandle<()>)> {
    let (outgoing, queued) = mpsc::channel::<EmbedderMessage>();
    let thread = std::thread::Builder::new()
        .name("rikka-gecko-write".into())
        .spawn(move || {
            for message in queued {
                if let Err(error) = write_frame(&mut stream, &message) {
                    tracing::debug!(%error, "failed to send message to engine");
                    return;
                }
            }
        })?;
    Ok((outgoing, thread))
}
