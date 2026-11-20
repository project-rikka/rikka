mod inflate;

use std::sync::{Arc, Weak};
use std::time::Duration;

use discord_core::LinkState;
use discord_wire::{Closing, Frame, Hello, ReadySession, commands};
use futures_channel::oneshot;
use rand::Rng;
use rikka_net::{Message, header};
use serde_json::Value;
use serde_json::value::RawValue;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use crate::identity::{Identity, ORIGIN, USER_AGENT};
use crate::rest::Token;
pub(crate) use inflate::Inflater;

const GATEWAY: &str = "wss://gateway.discord.gg";
const QUERY: &str = "/?encoding=json&v=9&compress=zlib-stream";
const LONGEST_BACKOFF: Duration = Duration::from_secs(60);

pub(crate) trait Listener: Send + Sync + 'static {
    fn dispatch(&self, event: &str, data: &str);

    fn link(&self, state: LinkState);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum First {
    Ready,
    SignedOut,
    Offline,
}

#[derive(Clone, Debug)]
pub(crate) struct Gateway {
    commands: mpsc::UnboundedSender<Value>,
    stop: Arc<watch::Sender<bool>>,
}

impl Gateway {
    pub(crate) fn start(
        runtime: &Handle,
        token: Arc<Token>,
        identity: Arc<Identity>,
        listener: Weak<dyn Listener>,
    ) -> (Gateway, oneshot::Receiver<First>) {
        let (commands, inbox) = mpsc::unbounded_channel();
        let (stop, stopped) = watch::channel(false);
        let (first, outcome) = oneshot::channel();
        let connection = Connection {
            client: rikka_net::Client::new(),
            token,
            identity,
            listener,
            inbox,
            stopped,
            first: Some(first),
            session: None,
            sequence: None,
            healthy: false,
        };
        runtime.spawn(connection.run());
        let gateway = Gateway {
            commands,
            stop: Arc::new(stop),
        };
        (gateway, outcome)
    }

    pub(crate) fn send(&self, payload: Value) {
        let _ = self.commands.send(payload);
    }

    pub(crate) fn close(&self) {
        let _ = self.stop.send(true);
    }
}

struct Session {
    id: String,
    resume_url: Option<String>,
}

enum Ended {
    Resume,
    Restart,
    SignedOut,
    Stopped,
}

impl From<Closing> for Ended {
    fn from(closing: Closing) -> Ended {
        match closing {
            Closing::Resume => Ended::Resume,
            Closing::Restart => Ended::Restart,
            Closing::SignedOut => Ended::SignedOut,
        }
    }
}

struct Connection {
    client: rikka_net::Client,
    token: Arc<Token>,
    identity: Arc<Identity>,
    listener: Weak<dyn Listener>,
    inbox: mpsc::UnboundedReceiver<Value>,
    stopped: watch::Receiver<bool>,
    first: Option<oneshot::Sender<First>>,
    session: Option<Session>,
    sequence: Option<u64>,
    healthy: bool,
}

struct Beat {
    every: Duration,
    next: Instant,
    acknowledged: bool,
}

impl Connection {
    async fn run(mut self) {
        let mut attempt = 0u32;
        loop {
            if attempt == 0 {
                self.announce(LinkState::Connecting);
            }
            let mut stopped = self.stopped.clone();
            let ended = tokio::select! {
                ended = self.connect_and_serve() => ended,
                _ = stopped.wait_for(|stop| *stop) => Ended::Stopped,
            };
            match ended {
                Ended::Stopped => return,
                Ended::SignedOut => {
                    self.resolve_first(First::SignedOut);
                    self.announce(LinkState::SignedOut);
                    return;
                }
                Ended::Restart => {
                    self.session = None;
                    self.sequence = None;
                }
                Ended::Resume => {}
            }
            if self.listener.strong_count() == 0 {
                return;
            }
            if std::mem::take(&mut self.healthy) {
                attempt = 0;
            }
            attempt += 1;
            self.announce(LinkState::Reconnecting { attempt });
            let mut stopped = self.stopped.clone();
            tokio::select! {
                () = tokio::time::sleep(backoff(attempt)) => {}
                _ = stopped.wait_for(|stop| *stop) => return,
            }
        }
    }

    async fn connect_and_serve(&mut self) -> Ended {
        match self.open().await {
            Ok(socket) => self.serve(socket).await,
            Err(error) => {
                tracing::debug!(%error, "Discord gateway unreachable");
                self.resolve_first(First::Offline);
                Ended::Resume
            }
        }
    }

    async fn open(&self) -> Result<rikka_net::WebSocket, rikka_net::Error> {
        let base = self
            .session
            .as_ref()
            .and_then(|session| session.resume_url.as_deref())
            .unwrap_or(GATEWAY)
            .trim_end_matches('/');
        self.client
            .websocket(format!("{base}{QUERY}"))
            .header(header::USER_AGENT, USER_AGENT)
            .header(header::ORIGIN, ORIGIN)
            .no_heartbeat()
            .connect()
            .await
    }

    async fn serve(&mut self, mut socket: rikka_net::WebSocket) -> Ended {
        let mut inflater = Inflater::new();
        let mut beat: Option<Beat> = None;
        let mut identified = false;
        loop {
            let wake = beat.as_ref().map_or_else(far_future, |beat| beat.next);
            tokio::select! {
                received = socket.recv() => {
                    let text = match received {
                        Some(Ok(Message::Binary(bytes))) => match inflater.push(&bytes) {
                            Ok(Some(text)) => text,
                            Ok(None) => continue,
                            Err(error) => {
                                tracing::warn!(%error, "dropping Discord gateway connection");
                                return Ended::Resume;
                            }
                        },
                        Some(Ok(Message::Text(text))) => text.into_bytes(),
                        Some(Ok(Message::Close(frame))) => {
                            return Closing::from_code(frame.map(|frame| frame.code.get())).into();
                        }
                        Some(Ok(_)) => continue,
                        Some(Err(error)) => {
                            tracing::debug!(%error, "Discord gateway connection dropped");
                            return Ended::Resume;
                        }
                        None => return Ended::Resume,
                    };
                    let Ok(frame) = serde_json::from_slice::<Frame<'_>>(&text) else {
                        tracing::debug!("skipping unreadable gateway payload");
                        continue;
                    };
                    if let Some(sequence) = frame.s {
                        self.sequence = Some(sequence);
                    }
                    match frame.op {
                        0 => {
                            let event = frame.t.unwrap_or_default();
                            let data = frame.d.map_or("null", RawValue::get);
                            if event == "READY" {
                                if let Ok(ready) = serde_json::from_str::<ReadySession>(data) {
                                    self.session = Some(Session {
                                        id: ready.session_id,
                                        resume_url: ready.resume_gateway_url,
                                    });
                                }
                                identified = true;
                                self.healthy = true;
                                self.resolve_first(First::Ready);
                                self.announce(LinkState::Online);
                            } else if event == "RESUMED" {
                                identified = true;
                                self.healthy = true;
                                self.resolve_first(First::Ready);
                                self.announce(LinkState::Online);
                            }
                            let Some(listener) = self.listener.upgrade() else {
                                return Ended::Stopped;
                            };
                            listener.dispatch(event, data);
                        }
                        1 => {
                            if send(&mut socket, &commands::heartbeat(self.sequence)).await.is_err() {
                                return Ended::Resume;
                            }
                        }
                        7 => return Ended::Resume,
                        9 => {
                            let resumable = frame.d.is_some_and(|data| data.get() == "true");
                            let pause = Duration::from_millis(rand::rng().random_range(1000..5000));
                            tokio::time::sleep(pause).await;
                            return if resumable { Ended::Resume } else { Ended::Restart };
                        }
                        10 => {
                            let every = frame
                                .d
                                .and_then(|data| serde_json::from_str::<Hello>(data.get()).ok())
                                .map_or(Duration::from_secs(41), |hello| Duration::from_millis(hello.heartbeat_interval));
                            let jitter: f64 = rand::rng().random_range(0.0..1.0);
                            beat = Some(Beat {
                                every,
                                next: Instant::now() + every.mul_f64(jitter),
                                acknowledged: true,
                            });
                            let greeting = match &self.session {
                                Some(session) => commands::resume(self.token.reveal(), &session.id, self.sequence),
                                None => commands::identify(self.token.reveal(), &self.identity.properties(true)),
                            };
                            if send(&mut socket, &greeting).await.is_err() {
                                return Ended::Resume;
                            }
                        }
                        11 => {
                            if let Some(beat) = beat.as_mut() {
                                beat.acknowledged = true;
                            }
                        }
                        _ => {}
                    }
                }
                command = self.inbox.recv(), if identified => {
                    let Some(payload) = command else {
                        let _ = socket.close().await;
                        return Ended::Stopped;
                    };
                    if send(&mut socket, &payload).await.is_err() {
                        return Ended::Resume;
                    }
                }
                () = tokio::time::sleep_until(wake), if beat.is_some() => {
                    let Some(state) = beat.as_mut() else {
                        continue;
                    };
                    if !state.acknowledged {
                        tracing::debug!("Discord gateway missed heartbeat ack");
                        let _ = socket.close().await;
                        return Ended::Resume;
                    }
                    state.acknowledged = false;
                    state.next = Instant::now() + state.every;
                    if send(&mut socket, &commands::heartbeat(self.sequence)).await.is_err() {
                        return Ended::Resume;
                    }
                }
            }
        }
    }

    fn announce(&self, state: LinkState) {
        if let Some(listener) = self.listener.upgrade() {
            listener.link(state);
        }
    }

    fn resolve_first(&mut self, outcome: First) {
        if let Some(first) = self.first.take() {
            let _ = first.send(outcome);
        }
    }
}

async fn send(socket: &mut rikka_net::WebSocket, payload: &Value) -> Result<(), rikka_net::Error> {
    socket.send_text(payload.to_string()).await
}

fn backoff(attempt: u32) -> Duration {
    let base = Duration::from_secs(1u64 << attempt.min(6)).min(LONGEST_BACKOFF);
    let jitter = Duration::from_millis(rand::rng().random_range(0..4000));
    base + jitter
}

fn far_future() -> Instant {
    Instant::now() + Duration::from_secs(24 * 60 * 60)
}
