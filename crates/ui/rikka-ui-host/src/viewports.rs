use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::thread::JoinHandle;

use rikka_renderer::Renderer;
use rikka_sync::Mutex;

use rikka_picture::Waking;
use rikka_ui::consume::any_offscreen_live;

use crate::offscreens::{OffscreenFrame, OffscreenStore};

pub(crate) struct Viewports {
    request: Option<SyncSender<()>>,
    drawn: Receiver<OffscreenFrame>,
    thread: Option<JoinHandle<()>>,
    in_flight: bool,
    held_last: bool,
}

impl Viewports {
    pub(crate) fn start(
        renderer: Arc<Mutex<Renderer>>,
        wake: Option<Waking>,
    ) -> std::io::Result<Self> {
        let (request, requests) = sync_channel::<()>(1);
        let (done, drawn) = channel::<OffscreenFrame>();
        let thread = std::thread::Builder::new()
            .name(String::from("rikka-ui-viewports"))
            .spawn(move || draw(&renderer, &requests, &done, wake.as_ref()))?;
        Ok(Self {
            request: Some(request),
            drawn,
            thread: Some(thread),
            in_flight: false,
            held_last: false,
        })
    }

    pub(crate) fn request_pass(&mut self, presentable: bool) {
        if self.in_flight || !presentable || !(any_offscreen_live() || self.held_last) {
            return;
        }
        let Some(request) = self.request.as_ref() else {
            return;
        };
        self.in_flight = request.try_send(()).is_ok();
    }
    pub(crate) fn take_drawn(&mut self) -> Option<OffscreenFrame> {
        match self.drawn.try_recv() {
            Ok(frame) => {
                self.in_flight = false;
                self.held_last = frame.holds_renders;
                Some(frame)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                None
            }
        }
    }
}

impl Drop for Viewports {
    fn drop(&mut self) {
        drop(self.request.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl std::fmt::Debug for Viewports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewports")
            .field("in_flight", &self.in_flight)
            .finish_non_exhaustive()
    }
}

fn draw(
    renderer: &Mutex<Renderer>,
    requests: &Receiver<()>,
    done: &Sender<OffscreenFrame>,
    wake: Option<&Waking>,
) {
    let mut store = OffscreenStore::new();
    while requests.recv().is_ok() {
        let frame = {
            let mut renderer = renderer.lock();
            store.advance(&mut renderer)
        };
        let worth_hearing = !frame.updated.is_empty() || frame.appeared;
        if done.send(frame).is_err() {
            break;
        }
        if worth_hearing && let Some(wake) = wake {
            wake();
        }
    }
}
