use std::collections::HashMap;
use std::io;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rikka_worker::Worker;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, watch};

use super::worker::{Hangup, runtime, started};

use crate::call::{Call, Responder};
use crate::client::SeamClient;
use crate::dispatcher::Dispatcher;
use crate::error::SeamError;
use crate::event::{EventBus, Lagged};
use crate::manifest::Manifest;
use crate::net::address::{session_sockets, socket_path};
use crate::net::frame::{self, Frame, OUTGOING};
use crate::registry::{CAPABILITIES, SEAM_CAPABILITY};
use crate::request::Request;

const IN_FLIGHT: usize = 512;

#[derive(Debug)]
pub struct SocketClient {
    shared: Arc<Shared>,
    _worker: Worker<Hangup>,
}

#[derive(Debug)]
struct Shared {
    waiting: Mutex<Option<HashMap<u64, Responder>>>,
    outgoing: mpsc::Sender<Frame>,
    next_id: AtomicU64,
    bus: EventBus,
    connected: watch::Sender<bool>,
}

impl SocketClient {
    pub fn from_env() -> io::Result<Self> {
        let path = socket_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no seam socket: neither $RIKKA_SEAM_SOCKET nor $XDG_RUNTIME_DIR is set",
            )
        })?;
        Self::connect(path)
    }

    pub fn for_capability(capability: &str) -> io::Result<Self> {
        let mut refused = Vec::new();
        for path in session_sockets() {
            match Self::provider(&path, capability) {
                Ok(Some(socket)) => return Ok(socket),
                Ok(None) => {}
                Err(error) => refused.push(format!("{}: {error}", path.display())),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            match refused.is_empty() {
                true => format!("nothing in this session serves `{capability}`"),
                false => format!(
                    "nothing reachable in this session serves `{capability}` ({})",
                    refused.join(", ")
                ),
            },
        ))
    }

    fn provider(path: &Path, capability: &str) -> io::Result<Option<Self>> {
        let socket = Self::connect(path)?;
        let provides = socket
            .manifests()
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(provides
            .iter()
            .any(|manifest| manifest.capability().as_str() == capability)
            .then_some(socket))
    }

    pub fn manifests(&self) -> Result<Vec<Manifest>, SeamError> {
        self.client()
            .call(Request::new(SEAM_CAPABILITY, CAPABILITIES))
            .blocking_wait()?
            .decode()
    }

    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let stream = StdUnixStream::connect(path.as_ref())?;
        stream.set_nonblocking(true)?;

        let (outgoing, queued) = mpsc::channel(OUTGOING);
        let shared = Arc::new(Shared {
            waiting: Mutex::new(Some(HashMap::new())),
            outgoing,
            next_id: AtomicU64::new(1),
            bus: EventBus::new(),
            connected: watch::Sender::new(true),
        });

        shared
            .outgoing
            .try_send(Frame::Subscribe { capability: None })
            .expect("new outgoing queue should have room for the subscription");

        let (stop, stopped) = oneshot::channel();
        let talking = Arc::clone(&shared);
        let worker = Worker::spawn("rikka-seam-client", Hangup::new(stop), move || {
            let runtime = runtime()?;
            let stream = {
                let _inside = runtime.enter();
                UnixStream::from_std(stream)?
            };
            Ok(move || runtime.block_on(converse(stream, talking, queued, stopped)))
        })
        .map_err(started)?;

        Ok(Self {
            shared,
            _worker: worker,
        })
    }

    #[must_use]
    pub fn client(&self) -> SeamClient {
        SeamClient::new(
            Arc::clone(&self.shared) as Arc<dyn Dispatcher>,
            self.shared.bus.clone(),
        )
    }

    #[must_use]
    pub fn dispatcher(&self) -> Arc<dyn Dispatcher> {
        Arc::clone(&self.shared) as Arc<dyn Dispatcher>
    }

    #[must_use]
    pub fn events(&self) -> crate::EventQueue {
        self.shared.bus.queue(None)
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        *self.shared.connected.borrow()
    }

    pub async fn closed(&self) {
        let mut connected = self.shared.connected.subscribe();
        if connected.wait_for(|connected| !connected).await.is_err() {
            tracing::trace!("seam connection state dropped");
        }
    }
}

impl Drop for SocketClient {
    fn drop(&mut self) {
        self.shared.close(SeamError::Disconnected);
    }
}

impl Dispatcher for Shared {
    fn enqueue(&self, call: Call) {
        let (request, responder) = call.defer();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (capability, method, payload, require) = request.into_parts();
        let frame = Frame::Call {
            id,
            capability,
            method: method.as_str().to_owned(),
            payload,
            require,
        };

        let refused = {
            let Ok(mut waiting) = self.waiting.lock() else {
                return responder.fail(SeamError::Disconnected);
            };
            match waiting.as_mut() {
                Some(map) if map.len() < IN_FLIGHT => {
                    map.insert(id, responder);
                    None
                }
                Some(_) => Some((responder, SeamError::Overloaded)),
                None => Some((responder, SeamError::Disconnected)),
            }
        };
        if let Some((responder, why)) = refused {
            return responder.fail(why);
        }

        match self.outgoing.try_send(frame) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.answer(id, Err(SeamError::Overloaded));
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.answer(id, Err(SeamError::Disconnected));
            }
        }
    }
}

impl Shared {
    fn answer(&self, id: u64, result: Result<crate::payload::Payload, SeamError>) {
        let responder = self
            .waiting
            .lock()
            .ok()
            .and_then(|mut waiting| waiting.as_mut()?.remove(&id));
        match responder {
            Some(responder) => responder.answer(result),
            None => tracing::debug!("reply for unknown request {id}"),
        }
    }

    fn close(&self, reason: SeamError) {
        if let Ok(mut waiting) = self.waiting.lock()
            && let Some(pending) = waiting.take()
        {
            for (_, responder) in pending {
                responder.fail(reason.clone());
            }
        }
        self.connected.send_replace(false);
        self.bus.close();
    }
}

async fn converse(
    stream: UnixStream,
    shared: Arc<Shared>,
    queued: mpsc::Receiver<Frame>,
    stopped: oneshot::Receiver<()>,
) {
    let (mut reading, writing) = stream.into_split();
    let writer = frame::writer(writing, queued);

    tokio::select! {
        () = deliver(&mut reading, &shared) => {}
        _ = stopped => {}
    }

    shared.close(SeamError::Disconnected);
    writer.abort();
    if let Err(error) = writer.await
        && !error.is_cancelled()
    {
        tracing::warn!(%error, "seam writer task failed");
    }
}

async fn deliver(reading: &mut tokio::net::unix::OwnedReadHalf, shared: &Shared) {
    loop {
        match frame::read(reading).await {
            Ok(Some(Frame::Reply { id, payload })) => shared.answer(id, Ok(payload)),
            Ok(Some(Frame::Fail { id, code, message })) => {
                shared.answer(id, Err(SeamError::Remote { code, message }));
            }
            Ok(Some(Frame::Event { event })) => shared.bus.send(event),
            Ok(Some(Frame::Lagged {
                capabilities,
                count,
            })) => shared.bus.lagged(&Lagged::new(capabilities, count)),
            Ok(Some(Frame::Call { .. } | Frame::Subscribe { .. })) => {
                tracing::debug!("unexpected request from seam socket");
                return;
            }
            Ok(None) => return,
            Err(error) => {
                tracing::debug!("invalid frame from seam socket: {error}");
                return;
            }
        }
    }
}
