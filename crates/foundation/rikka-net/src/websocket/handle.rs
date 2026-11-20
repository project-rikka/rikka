use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

use tokio::sync::{Notify, mpsc};

use crate::error::{Error, Result};
use crate::ready_state::ReadyState;
use crate::websocket::builder::WebSocketBuilder;
use crate::websocket::message::Message;

const OUTGOING: usize = 1024;

const INCOMING: usize = 4096;
const INCOMING_BYTES: usize = 64 << 20;

pub struct SocketHandle {
    shared: Arc<Shared>,
    outgoing: mpsc::Sender<Message>,
}

#[derive(Default)]
struct Incoming {
    messages: VecDeque<Message>,
    bytes: usize,
}

struct Shared {
    incoming: Mutex<Incoming>,
    state: Mutex<ReadyState>,
    error: Mutex<Option<Arc<Error>>>,
    watcher: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    closing: Notify,
}

impl Shared {
    fn set_state(&self, state: ReadyState) {
        *self.state.lock() = state;
        self.wake();
    }

    fn record_error(&self, error: Error) {
        *self.error.lock() = Some(Arc::new(error));
    }

    fn received(&self, message: Message) -> bool {
        let room = {
            let mut incoming = self.incoming.lock();
            if incoming.messages.len() >= INCOMING || incoming.bytes >= INCOMING_BYTES {
                false
            } else {
                incoming.bytes += message.len();
                incoming.messages.push_back(message);
                true
            }
        };
        self.wake();
        room
    }

    fn wake(&self) {
        let watcher = self.watcher.lock().clone();
        if let Some(watcher) = watcher {
            watcher();
        }
    }
}

impl SocketHandle {
    pub(crate) fn spawn(builder: WebSocketBuilder) -> SocketHandle {
        let (outgoing, commands) = mpsc::channel(OUTGOING);
        let shared = Arc::new(Shared {
            incoming: Mutex::new(Incoming::default()),
            state: Mutex::new(ReadyState::Connecting),
            error: Mutex::new(None),
            watcher: Mutex::new(None),
            closing: Notify::new(),
        });

        crate::runtime::shared().spawn(run_socket(builder, Arc::clone(&shared), commands));
        SocketHandle { shared, outgoing }
    }

    #[must_use]
    pub fn state(&self) -> ReadyState {
        *self.shared.state.lock()
    }

    #[must_use]
    pub fn error(&self) -> Option<Arc<Error>> {
        self.shared.error.lock().clone()
    }

    #[must_use]
    pub fn try_recv(&self) -> Option<Message> {
        let mut incoming = self.shared.incoming.lock();
        let message = incoming.messages.pop_front()?;
        incoming.bytes -= message.len();
        Some(message)
    }

    pub fn send(&self, message: impl Into<Message>) -> Result<()> {
        self.outgoing
            .try_send(message.into())
            .map_err(|failed| match failed {
                mpsc::error::TrySendError::Full(_) => Error::Backpressure,
                mpsc::error::TrySendError::Closed(_) => Error::ConnectionClosed,
            })
    }

    pub fn close(&self) -> Result<()> {
        if self.state() == ReadyState::Closed {
            return Err(Error::ConnectionClosed);
        }
        self.shared.closing.notify_one();
        Ok(())
    }

    pub fn on_event(&self, watcher: impl Fn() + Send + Sync + 'static) {
        *self.shared.watcher.lock() = Some(Arc::new(watcher));
        self.shared.wake();
    }
}

impl fmt::Debug for SocketHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SocketHandle")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

async fn run_socket(
    builder: WebSocketBuilder,
    shared: Arc<Shared>,
    mut queued: mpsc::Receiver<Message>,
) {
    let mut socket = match builder.connect().await {
        Ok(socket) => socket,
        Err(error) => {
            shared.record_error(error);
            shared.set_state(ReadyState::Closed);
            return;
        }
    };
    let mut state = socket.state();
    shared.set_state(state);

    loop {
        let step = tokio::select! {
            () = shared.closing.notified() => Step::Closing,
            sending = queued.recv() => Step::Sending(sending),
            arrived = socket.recv() => Step::Arrived(arrived),
        };

        match step {
            Step::Closing | Step::Sending(None) => {
                if let Err(error) = socket.close().await {
                    shared.record_error(error);
                }
                break;
            }
            Step::Sending(Some(message)) => {
                if let Err(error) = socket.send(message).await {
                    shared.record_error(error);
                    break;
                }
            }
            Step::Arrived(None) => break,
            Step::Arrived(Some(Ok(message))) => {
                if !shared.received(message) {
                    shared.record_error(Error::Backpressure);
                    let _ = socket.close().await;
                    break;
                }
            }
            Step::Arrived(Some(Err(error))) => {
                shared.record_error(error);
                break;
            }
        }

        let now = socket.state();
        if now != state {
            state = now;
            shared.set_state(state);
        }
    }

    shared.set_state(ReadyState::Closed);
}

enum Step {
    Closing,
    Sending(Option<Message>),
    Arrived(Option<Result<Message>>),
}
