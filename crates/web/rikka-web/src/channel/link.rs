use std::collections::HashMap;
use std::future::Future;
use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::{Context, Poll};
use std::thread::JoinHandle;

use futures_channel::oneshot;
use rikka_sync::Mutex;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::channel::frame;
use crate::channel::message::Outgoing;
use crate::error::{Error, Result};

type Continuation = Box<dyn FnOnce(Result<Value>) + Send>;

enum Write {
    Stream(Option<UnixStream>),
    Frame(Vec<u8>),
}

enum Pending {
    Awaited(oneshot::Sender<Result<Value>>),
    Continued(Continuation),
}

impl Pending {
    fn settle(self, result: Result<Value>) {
        match self {
            Pending::Awaited(sender) => {
                let _ = sender.send(result);
            }
            Pending::Continued(continuation) => continuation(result),
        }
    }
}

#[derive(Default)]
struct State {
    ready: bool,
    ended: bool,
    refusing: bool,
    queued: Vec<Vec<u8>>,
    next: u64,
    pending: HashMap<u64, Pending>,
}

pub(crate) struct Link {
    state: Mutex<State>,
    writer: mpsc::Sender<Write>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.lock();
        formatter
            .debug_struct("Link")
            .field("ready", &state.ready)
            .field("pending", &state.pending.len())
            .finish_non_exhaustive()
    }
}

#[must_use = "a call is only observed when awaited"]
pub(crate) struct Called(oneshot::Receiver<Result<Value>>);

impl Future for Called {
    type Output = Result<Value>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Result<Value>> {
        Pin::new(&mut self.0)
            .poll(context)
            .map(|answer| answer.unwrap_or(Err(Error::Disconnected)))
    }
}

impl Called {
    pub(crate) async fn decoded<T: DeserializeOwned>(self) -> Result<T> {
        decode(self.await)
    }

    pub(crate) async fn done(self) -> Result<()> {
        self.await.map(|_| ())
    }
}

pub(crate) fn decode<T: DeserializeOwned>(result: Result<Value>) -> Result<T> {
    serde_json::from_value(result?).map_err(|error| Error::Malformed(error.to_string()))
}

impl Link {
    pub(crate) fn start() -> std::io::Result<(Link, JoinHandle<()>)> {
        let (writer, writes) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rikka-web-write".into())
            .spawn(move || write_loop(writes))?;
        Ok((
            Link {
                state: Mutex::new(State::default()),
                writer,
            },
            thread,
        ))
    }

    pub(crate) fn call(&self, api: &str, args: Vec<Value>) -> Called {
        let (sender, receiver) = oneshot::channel();
        self.issue(api, &args, Pending::Awaited(sender));
        Called(receiver)
    }

    pub(crate) fn call_then(
        &self,
        api: &str,
        args: Vec<Value>,
        continuation: impl FnOnce(Result<Value>) + Send + 'static,
    ) {
        self.issue(api, &args, Pending::Continued(Box::new(continuation)));
    }

    pub(crate) fn notify(&self, api: &str, args: Vec<Value>) {
        let api_name = api.to_owned();
        self.call_then(api, args, move |result| {
            if let Err(error) = result {
                tracing::debug!(%error, api = api_name, "call failed");
            }
        });
    }

    fn issue(&self, api: &str, args: &[Value], pending: Pending) {
        let mut state = self.state.lock();
        let refused = match (state.ended, state.ready, state.refusing) {
            (true, _, _) => Some(Error::EngineExited),
            (false, false, true) => Some(Error::Disconnected),
            _ => None,
        };
        if let Some(error) = refused {
            drop(state);
            pending.settle(Err(error));
            return;
        }
        let body = serde_json::to_vec(&Outgoing::Call {
            call: state.next + 1,
            api,
            args,
        })
        .expect("JSON values always encode");
        let Some(frame) = frame::encode(&body) else {
            drop(state);
            pending.settle(Err(Error::Rejected(format!(
                "{api} exceeds the message limit"
            ))));
            return;
        };
        state.next += 1;
        let id = state.next;
        state.pending.insert(id, pending);
        if state.ready {
            self.write(frame);
        } else {
            state.queued.push(frame);
        }
    }

    pub(crate) fn answer(&self, ask: u64, value: &Value) {
        let state = self.state.lock();
        if !state.ready {
            return;
        }
        let body = serde_json::to_vec(&Outgoing::Answer { answer: ask, value })
            .expect("JSON values always encode");
        match frame::encode(&body) {
            Some(frame) => self.write(frame),
            None => tracing::warn!("answer exceeds the message limit"),
        }
    }

    pub(crate) fn connected(&self, stream: UnixStream) {
        self.send(Write::Stream(Some(stream)));
    }

    pub(crate) fn ready(&self) {
        let mut state = self.state.lock();
        state.ready = true;
        state.refusing = false;
        for frame in std::mem::take(&mut state.queued) {
            self.write(frame);
        }
    }

    pub(crate) fn disconnected(&self) {
        self.fail(false);
    }

    pub(crate) fn end(&self) {
        self.fail(true);
    }

    pub(crate) fn refuse(&self) {
        let pending = {
            let mut state = self.state.lock();
            if state.ready {
                return;
            }
            state.refusing = true;
            state.queued.clear();
            std::mem::take(&mut state.pending)
        };
        for (_, pending) in pending {
            pending.settle(Err(Error::Disconnected));
        }
    }

    fn fail(&self, ended: bool) {
        let pending = {
            let mut state = self.state.lock();
            state.ready = false;
            state.ended |= ended;
            state.queued.clear();
            std::mem::take(&mut state.pending)
        };
        self.send(Write::Stream(None));
        let error = || match ended {
            true => Error::EngineExited,
            false => Error::Disconnected,
        };
        for (_, pending) in pending {
            pending.settle(Err(error()));
        }
    }

    pub(crate) fn replied(&self, id: u64, result: Result<Value>) {
        let pending = self.state.lock().pending.remove(&id);
        match pending {
            Some(pending) => pending.settle(result),
            None => tracing::debug!(id, "reply to an unknown call"),
        }
    }

    fn write(&self, frame: Vec<u8>) {
        self.send(Write::Frame(frame));
    }

    fn send(&self, write: Write) {
        if self.writer.send(write).is_err() {
            tracing::debug!("extension writer already stopped");
        }
    }
}

fn write_loop(writes: mpsc::Receiver<Write>) {
    let mut stream: Option<UnixStream> = None;
    for write in writes {
        match write {
            Write::Stream(next) => stream = next,
            Write::Frame(frame) => {
                let Some(current) = stream.as_mut() else {
                    continue;
                };
                if let Err(error) = current.write_all(&frame) {
                    tracing::warn!(%error, "failed to write to the Rikka extension");
                    stream = None;
                }
            }
        }
    }
}
