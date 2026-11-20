use std::io;
use std::os::unix::net::UnixStream;
use std::thread::JoinHandle;

use rikka_gecko_protocol::{FrameError, GeckoMessage, read_frame};

pub(crate) fn spawn(
    mut stream: UnixStream,
    mut on_message: impl FnMut(GeckoMessage) + Send + 'static,
    ended: impl FnOnce(String) + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("rikka-gecko-read".into())
        .spawn(move || {
            let reason = loop {
                match read_frame::<_, GeckoMessage>(&mut stream) {
                    Ok(Some(message)) => on_message(message),
                    Ok(None) => break "engine closed the channel".to_owned(),
                    Err(FrameError::Json(error)) => {
                        tracing::warn!(%error, "unknown engine message");
                    }
                    Err(error) => break error.to_string(),
                }
            };
            ended(reason);
        })
}
