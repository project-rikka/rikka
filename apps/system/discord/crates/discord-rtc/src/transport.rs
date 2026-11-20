use std::sync::Arc;

use discord_core::{EncodedVideo, VideoShape, VoiceSink};
use futures_channel::oneshot;

use crate::endpoint::{Endpoint, Role};
use crate::outbox::Outbox;
use crate::session;

const THREAD: &str = "rikka-discord-rtc";

pub struct Transport {
    outbox: Arc<Outbox>,
}

impl Transport {
    pub async fn open(
        endpoint: Endpoint,
        role: Role,
        sink: VoiceSink,
    ) -> discord_core::Result<Transport> {
        let outbox = Arc::new(Outbox::default());
        let (open_reply, opening) = oneshot::channel();
        let worker = Arc::clone(&outbox);
        std::thread::Builder::new()
            .name(THREAD.to_owned())
            .spawn(move || session::run(endpoint, role, sink, worker, open_reply))
            .map_err(discord_core::Error::voice)?;
        match opening.await {
            Ok(Ok(())) => Ok(Transport { outbox }),
            Ok(Err(error)) => Err(error.into()),
            Err(oneshot::Canceled) => {
                Err(discord_core::Error::voice("the voice connection stopped"))
            }
        }
    }

    pub fn send(&self, opus: &[u8]) {
        self.outbox.frame(opus);
    }

    pub fn speaking(&self, on: bool) {
        self.outbox.speaking(on);
    }

    pub fn send_video(&self, frame: EncodedVideo) {
        self.outbox.video(frame);
    }

    pub fn show_video(&self, shape: Option<VideoShape>) {
        self.outbox.shape(shape);
    }

    pub fn refresh(&self, source: u32) {
        self.outbox.refresh(source);
    }

    pub fn close(&self) {
        self.outbox.close();
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.close();
    }
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Transport")
            .field("closed", &self.outbox.is_closed())
            .finish()
    }
}
