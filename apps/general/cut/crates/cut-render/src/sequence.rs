use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::time::{Duration, Instant};

use rikka_image_edit::Imaging;
use rikka_types::CancelToken;

use crate::compositor::{Composite, Compositor};
use crate::error::{Error, Result};
use crate::request::Request;

const FRAME_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Sequence {
    compositor: Compositor,
    cancel: CancelToken,
    wake: Receiver<()>,
}

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequence")
            .field("compositor", &self.compositor)
            .field("cancel", &self.cancel)
            .finish_non_exhaustive()
    }
}

impl Sequence {
    #[must_use]
    pub fn new(imaging: Imaging, cancel: CancelToken) -> Sequence {
        let mut compositor = Compositor::new(imaging);
        let (notify, wake) = sync_channel(1);
        let on_cancel = notify.clone();
        compositor.on_ready(move || {
            let _ = notify.try_send(());
        });
        cancel.on_cancel(move || {
            let _ = on_cancel.try_send(());
        });
        Sequence {
            compositor,
            cancel,
            wake,
        }
    }

    pub fn render(&mut self, request: &Request) -> Result<Option<Composite>> {
        let frame = request.frame();
        let deadline = Instant::now() + FRAME_TIMEOUT;
        loop {
            if self.cancel.is_cancelled() {
                return Ok(None);
            }
            let inputs = self.compositor.collect_inputs(request, frame);
            if inputs.is_ready() {
                return self.compositor.compose(request, frame, &inputs).map(Some);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.wake.recv_timeout(left) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => return Err(Error::Stalled { frame }),
                Err(RecvTimeoutError::Disconnected) => unreachable!("the sequence owns a sender"),
            }
        }
    }
}
