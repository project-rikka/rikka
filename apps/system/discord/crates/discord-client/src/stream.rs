use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use discord_core::{
    ChannelId, EncodedVideo, SpaceId, StreamKey, UserId, VideoShape, VoiceEvent, VoiceEvents,
    VoiceSink,
};
use discord_rtc::{Endpoint, Role, Transport};
use rikka_sync::Mutex;
use tokio::runtime::Handle;
use tokio::sync::Notify;

use discord_state::StreamSignal;
use discord_wire::{commands, snowflake};

use crate::error::{Error, Result};
use crate::session::Session;

const ANSWER_WITHIN: Duration = Duration::from_secs(20);

#[derive(Default)]
pub(crate) struct Streams {
    inner: Mutex<Inner>,
    changed: Notify,
}

#[derive(Default)]
struct Inner {
    known: HashMap<StreamKey, Known>,
    active: HashMap<StreamKey, Arc<Active>>,
}

#[derive(Clone, Default)]
struct Known {
    created: Option<Created>,
    server: Option<Server>,
    deleted: Option<Deleted>,
}

#[derive(Clone)]
struct Created {
    server: String,
    channel: Option<String>,
}

#[derive(Clone)]
struct Server {
    token: String,
    endpoint: Option<String>,
}

#[derive(Clone)]
struct Deleted {
    reason: Option<String>,
    unavailable: bool,
}

#[derive(Clone, Copy)]
struct Target {
    server: u64,
    channel: ChannelId,
}

struct Active {
    key: StreamKey,
    role: Role,
    user: UserId,
    session: String,
    target: Mutex<Target>,
    sink: VoiceSink,
    transport: Mutex<Option<Arc<Transport>>>,
    leaving: AtomicBool,
    runtime: Handle,
}

impl Active {
    fn endpoint(&self, token: String, endpoint: String) -> Endpoint {
        let target = *self.target.lock();
        Endpoint {
            endpoint,
            token,
            server: target.server,
            channel: target.channel,
            group: target.server.saturating_sub(1),
            user: self.user,
            session: self.session.clone(),
        }
    }

    fn transport(&self) -> Option<Arc<Transport>> {
        self.transport.lock().clone()
    }

    fn shut(&self) {
        self.leaving.store(true, Ordering::SeqCst);
        if let Some(transport) = self.transport.lock().take() {
            transport.close();
        }
    }

    fn suspend(&self) {
        if let Some(transport) = self.transport.lock().take() {
            transport.close();
        }
    }

    fn reopen(self: &Arc<Self>, token: String, endpoint: String) {
        let active = Arc::clone(self);
        self.runtime.spawn(async move {
            active.suspend();
            let target = active.endpoint(token, endpoint);
            match Transport::open(target, active.role, active.sink.clone()).await {
                Ok(transport) if !active.leaving.load(Ordering::SeqCst) => {
                    *active.transport.lock() = Some(Arc::new(transport));
                }
                Ok(transport) => transport.close(),
                Err(error) => active.sink.send(VoiceEvent::Ended(Some(error))),
            }
        });
    }
}

pub struct Broadcast {
    pub key: StreamKey,
    pub line: BroadcastLine,
    pub events: VoiceEvents,
}

pub struct Watch {
    pub key: StreamKey,
    pub line: WatchLine,
    pub events: VoiceEvents,
}

impl std::fmt::Debug for Broadcast {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Broadcast")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Watch")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BroadcastLine {
    active: Arc<Active>,
    account: Weak<Session>,
}

#[derive(Clone)]
pub struct WatchLine {
    active: Arc<Active>,
    account: Weak<Session>,
}

impl std::fmt::Debug for BroadcastLine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BroadcastLine")
            .field("key", &self.active.key)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for WatchLine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WatchLine")
            .field("key", &self.active.key)
            .finish_non_exhaustive()
    }
}

impl BroadcastLine {
    pub fn send_video(&self, frame: EncodedVideo) {
        if let Some(transport) = self.active.transport() {
            transport.send_video(frame);
        }
    }

    pub fn show_video(&self, shape: Option<VideoShape>) {
        if let Some(transport) = self.active.transport() {
            transport.show_video(shape);
        }
    }

    pub fn send_sound(&self, opus: &[u8]) {
        if let Some(transport) = self.active.transport() {
            transport.send(opus);
        }
    }

    pub fn sound_speaking(&self, on: bool) {
        if let Some(transport) = self.active.transport() {
            transport.speaking(on);
        }
    }

    pub fn pause(&self, paused: bool) {
        if let Some(account) = self.account.upgrade() {
            account.send(commands::stream_paused(self.active.key, paused));
        }
    }

    pub fn end(&self) {
        end(&self.active, &self.account);
    }
}

impl WatchLine {
    pub fn refresh(&self, source: u32) {
        if let Some(transport) = self.active.transport() {
            transport.refresh(source);
        }
    }

    pub fn leave(&self) {
        end(&self.active, &self.account);
    }
}

fn end(active: &Arc<Active>, account: &Weak<Session>) {
    if active.leaving.load(Ordering::SeqCst) {
        return;
    }
    active.shut();
    if let Some(account) = account.upgrade() {
        account.streams().forget(active);
        account.send(commands::stream_delete(active.key));
    }
}

impl Streams {
    pub(crate) fn signal(&self, signal: StreamSignal) {
        let mut inner = self.inner.lock();
        match signal {
            StreamSignal::Created {
                key,
                server,
                channel,
                viewers,
                paused,
            } => {
                if let Some(active) = inner.active.get(&key) {
                    if let Some(server) = server {
                        active.target.lock().server = snowflake::value(&server);
                    }
                    let viewers = viewers.iter().map(|id| snowflake::id(id)).collect();
                    active.sink.send(VoiceEvent::Viewers(viewers));
                    active.sink.send(VoiceEvent::Paused(paused));
                    return;
                }
                let known = inner.known.entry(key).or_default();
                known.deleted = None;
                if let Some(server) = server {
                    known.created = Some(Created { server, channel });
                }
            }
            StreamSignal::Server {
                key,
                token,
                endpoint,
            } => {
                if let Some(active) = inner.active.get(&key) {
                    match endpoint {
                        Some(endpoint) => active.reopen(token, endpoint),
                        None => active.suspend(),
                    }
                    return;
                }
                inner.known.entry(key).or_default().server = Some(Server { token, endpoint });
            }
            StreamSignal::Deleted {
                key,
                reason,
                unavailable,
            } => {
                if let Some(active) = inner.active.remove(&key) {
                    if !active.leaving.load(Ordering::SeqCst) {
                        active.shut();
                        active
                            .sink
                            .send(VoiceEvent::Ended(ending(reason.as_deref(), unavailable)));
                    }
                    return;
                }
                inner.known.entry(key).or_default().deleted = Some(Deleted {
                    reason,
                    unavailable,
                });
            }
        }
        drop(inner);
        self.changed.notify_waiters();
    }

    pub(crate) fn end_all(&self, account: &Session) {
        let ended: Vec<Arc<Active>> = self
            .inner
            .lock()
            .active
            .drain()
            .map(|(_, active)| active)
            .collect();
        for active in ended {
            if active.leaving.load(Ordering::SeqCst) {
                continue;
            }
            active.shut();
            account.send(commands::stream_delete(active.key));
            active.sink.send(VoiceEvent::Ended(None));
        }
    }

    fn forget(&self, active: &Arc<Active>) {
        let mut inner = self.inner.lock();
        if inner
            .active
            .get(&active.key)
            .is_some_and(|known| Arc::ptr_eq(known, active))
        {
            inner.active.remove(&active.key);
        }
    }

    pub(crate) async fn go_live(account: Arc<Session>, channel: ChannelId) -> Result<Broadcast> {
        let seat = account
            .voice()
            .seat()
            .filter(|seat| snowflake::id::<ChannelId>(&seat.channel) == channel)
            .ok_or_else(|| Error::Voice("join the voice channel before going live".into()))?;
        let me = snowflake::id::<UserId>(account.state().me());
        let key = StreamKey {
            space: seat.guild.as_deref().map(snowflake::id::<SpaceId>),
            channel,
            owner: me,
        };
        account.streams().prepare(key);
        account.send(commands::stream_create(
            seat.guild.as_deref(),
            &seat.channel,
        ));
        account.send(commands::stream_paused(key, false));
        let (active, events) =
            Streams::open(&account, Role::Broadcast, key, channel, me, seat.session).await?;
        Ok(Broadcast {
            key,
            line: BroadcastLine {
                active,
                account: Arc::downgrade(&account),
            },
            events,
        })
    }

    pub(crate) async fn watch(account: Arc<Session>, key: StreamKey) -> Result<Watch> {
        let seat = account
            .voice()
            .seat()
            .filter(|seat| snowflake::id::<ChannelId>(&seat.channel) == key.channel)
            .ok_or_else(|| Error::Voice("join the voice channel to watch its streams".into()))?;
        let me = snowflake::id::<UserId>(account.state().me());
        account.streams().prepare(key);
        account.send(commands::stream_watch(key));
        let (active, events) =
            Streams::open(&account, Role::Watch, key, key.channel, me, seat.session).await?;
        Ok(Watch {
            key,
            line: WatchLine {
                active,
                account: Arc::downgrade(&account),
            },
            events,
        })
    }

    fn prepare(&self, key: StreamKey) {
        let mut inner = self.inner.lock();
        inner.known.remove(&key);
        if let Some(previous) = inner.active.remove(&key) {
            previous.shut();
        }
    }

    async fn open(
        account: &Arc<Session>,
        role: Role,
        key: StreamKey,
        channel: ChannelId,
        user: UserId,
        session: String,
    ) -> Result<(Arc<Active>, VoiceEvents)> {
        let streams = account.streams();
        let answered = tokio::time::timeout(ANSWER_WITHIN, streams.answered(key)).await;
        streams.inner.lock().known.remove(&key);
        let (created, server, endpoint) = match answered {
            Ok(Ok(answer)) => answer,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                account.send(commands::stream_delete(key));
                return Err(Error::Voice(
                    "Discord did not answer the stream request in time".into(),
                ));
            }
        };
        let (sink, events) = VoiceSink::new();
        let active = Arc::new(Active {
            key,
            role,
            user,
            session,
            target: Mutex::new(Target {
                server: snowflake::value(&created.server),
                channel: created
                    .channel
                    .as_deref()
                    .map_or(channel, snowflake::id::<ChannelId>),
            }),
            sink: sink.clone(),
            transport: Mutex::new(None),
            leaving: AtomicBool::new(false),
            runtime: account.runtime().clone(),
        });
        let target = active.endpoint(server.token, endpoint);
        let transport = match Transport::open(target, role, sink).await {
            Ok(transport) => transport,
            Err(error) => {
                account.send(commands::stream_delete(key));
                return Err(error.into());
            }
        };
        *active.transport.lock() = Some(Arc::new(transport));
        streams.inner.lock().active.insert(key, Arc::clone(&active));
        Ok((active, events))
    }

    async fn answered(&self, key: StreamKey) -> Result<(Created, Server, String)> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(known) = self.inner.lock().known.get(&key).cloned() {
                if let Some(deleted) = known.deleted {
                    let refused = ending(deleted.reason.as_deref(), deleted.unavailable);
                    return Err(refused.map_or_else(
                        || Error::Voice("the stream ended before it could be joined".into()),
                        Error::from,
                    ));
                }
                if let (Some(created), Some(server)) = (known.created, known.server)
                    && let Some(endpoint) = server.endpoint.clone()
                {
                    return Ok((created, server, endpoint));
                }
            }
            notified.await;
        }
    }
}

fn ending(reason: Option<&str>, unavailable: bool) -> Option<discord_core::Error> {
    if unavailable {
        return Some(discord_core::Error::Unreachable(
            "the stream is unavailable during an outage".into(),
        ));
    }
    match reason {
        None | Some("user_requested" | "stream_ended") => None,
        Some("stream_full") => Some(discord_core::Error::Denied("the stream is full".into())),
        Some("unauthorized") => Some(discord_core::Error::Denied(
            "this account may not watch the stream".into(),
        )),
        Some("safety_guild_rate_limited") => Some(discord_core::Error::Denied(
            "this server streams too often right now".into(),
        )),
        Some(other) => Some(discord_core::Error::Voice(other.replace('_', " "))),
    }
}
