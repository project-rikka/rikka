use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use discord_core::{ChannelId, UserId, VoiceEvent, VoiceEvents, VoiceSink};
use discord_rtc::{Endpoint, Role, Transport};
use rikka_sync::Mutex;
use serde_json::json;
use tokio::runtime::Handle;
use tokio::sync::Notify;

use discord_state::Signal;
use discord_wire::{commands, snowflake};

use crate::error::{Error, Result};
use crate::session::Session;

const ANSWER_WITHIN: Duration = Duration::from_secs(20);

#[derive(Default)]
pub(crate) struct Voice {
    inner: Mutex<Inner>,
    changed: Notify,
}

#[derive(Default)]
struct Inner {
    active: Option<Arc<Active>>,
    session: Option<VoiceSession>,
    server: Option<Server>,
}

#[derive(Clone)]
struct VoiceSession {
    guild: Option<String>,
    channel: Option<String>,
    session: String,
}

#[derive(Clone)]
struct Server {
    guild: Option<String>,
    channel: Option<String>,
    token: String,
    endpoint: Option<String>,
}

struct Active {
    guild: Option<String>,
    server: u64,
    user: UserId,
    channel: Mutex<String>,
    session: Mutex<String>,
    sink: VoiceSink,
    transport: Mutex<Option<Arc<Transport>>>,
    leaving: AtomicBool,
    runtime: Handle,
}

impl Active {
    fn hosts(&self, guild: Option<&str>, channel: Option<&str>) -> bool {
        match (&self.guild, guild) {
            (Some(mine), Some(theirs)) => mine == theirs,
            (None, None) => channel.is_none_or(|channel| *self.channel.lock() == channel),
            _ => false,
        }
    }

    fn shut(&self) {
        self.leaving.store(true, Ordering::SeqCst);
        if let Some(transport) = self.transport.lock().take() {
            transport.close();
        }
    }

    fn reopen(self: &Arc<Self>, token: String, endpoint: String) {
        let active = Arc::clone(self);
        self.runtime.spawn(async move {
            if let Some(old) = active.transport.lock().take() {
                old.close();
            }
            let channel = snowflake::id(&active.channel.lock());
            let target = Endpoint {
                endpoint,
                token,
                server: active.server,
                channel,
                group: ChannelId::get(channel),
                user: active.user,
                session: active.session.lock().clone(),
            };
            match Transport::open(target, Role::Voice, active.sink.clone()).await {
                Ok(transport) if !active.leaving.load(Ordering::SeqCst) => {
                    *active.transport.lock() = Some(Arc::new(transport));
                }
                Ok(transport) => transport.close(),
                Err(error) => active.sink.send(VoiceEvent::Ended(Some(error))),
            }
        });
    }
}

pub struct Call {
    pub channel: ChannelId,
    pub line: Line,
    pub events: VoiceEvents,
}

impl std::fmt::Debug for Call {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Call")
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

pub struct Line {
    active: Arc<Active>,
    account: Weak<Session>,
}

impl std::fmt::Debug for Line {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Line")
            .field("channel", &*self.active.channel.lock())
            .finish_non_exhaustive()
    }
}

impl Line {
    pub fn send(&self, opus: &[u8]) {
        if let Some(transport) = self.active.transport.lock().as_ref() {
            transport.send(opus);
        }
    }

    pub fn speaking(&self, on: bool) {
        if let Some(transport) = self.active.transport.lock().as_ref() {
            transport.speaking(on);
        }
    }

    pub fn set_self(&self, muted: bool, deafened: bool) {
        if let Some(account) = self.account.upgrade() {
            let channel = self.active.channel.lock().clone();
            account.send(commands::voice_state(
                self.active.guild.as_deref(),
                Some(&channel),
                muted,
                deafened,
            ));
        }
    }

    pub fn leave(&self) {
        if self.active.leaving.load(Ordering::SeqCst) {
            return;
        }
        self.active.shut();
        if let Some(account) = self.account.upgrade() {
            account.streams().end_all(&account);
            account.voice().forget(&self.active);
            account.send(commands::voice_state(
                self.active.guild.as_deref(),
                None,
                false,
                false,
            ));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Seat {
    pub(crate) guild: Option<String>,
    pub(crate) channel: String,
    pub(crate) session: String,
}

impl Voice {
    pub(crate) fn seat(&self) -> Option<Seat> {
        let active = self.inner.lock().active.clone()?;
        if active.leaving.load(Ordering::SeqCst) {
            return None;
        }
        let channel = active.channel.lock().clone();
        let session = active.session.lock().clone();
        Some(Seat {
            guild: active.guild.clone(),
            channel,
            session,
        })
    }

    pub(crate) fn signal(&self, signal: Signal) {
        let mut inner = self.inner.lock();
        match signal {
            Signal::Session {
                guild,
                channel,
                session,
            } => {
                if let Some(active) = inner.active.clone()
                    && active.hosts(guild.as_deref(), None)
                {
                    *active.session.lock() = session.clone();
                    match channel.as_deref() {
                        None => {
                            inner.active = None;
                            if !active.leaving.load(Ordering::SeqCst) {
                                active.shut();
                                active.sink.send(VoiceEvent::Ended(None));
                            }
                        }
                        Some(moved) if *active.channel.lock() != moved => {
                            *active.channel.lock() = moved.to_owned();
                            active.sink.send(VoiceEvent::Moved(snowflake::id(moved)));
                        }
                        Some(_) => {}
                    }
                }
                inner.session = Some(VoiceSession {
                    guild,
                    channel,
                    session,
                });
            }
            Signal::Server {
                guild,
                channel,
                token,
                endpoint,
            } => {
                if let Some(active) = inner.active.clone()
                    && active.hosts(guild.as_deref(), channel.as_deref())
                    && let Some(endpoint) = endpoint.clone()
                {
                    active.reopen(token.clone(), endpoint);
                }
                inner.server = Some(Server {
                    guild,
                    channel,
                    token,
                    endpoint,
                });
            }
        }
        drop(inner);
        self.changed.notify_waiters();
    }

    pub(crate) fn hang_up(&self, account: &Session) {
        let Some(active) = self.inner.lock().active.take() else {
            return;
        };
        account.streams().end_all(account);
        active.shut();
        account.send(commands::voice_state(
            active.guild.as_deref(),
            None,
            false,
            false,
        ));
    }

    fn forget(&self, active: &Arc<Active>) {
        let mut inner = self.inner.lock();
        if inner
            .active
            .as_ref()
            .is_some_and(|known| Arc::ptr_eq(known, active))
        {
            inner.active = None;
        }
    }

    pub(crate) async fn join(
        account: Arc<Session>,
        channel: String,
        muted: bool,
        deafened: bool,
    ) -> Result<Call> {
        let (guild, private, running, me) = {
            let state = account.state();
            (
                state.guild_of(&channel),
                state.is_private(&channel),
                state.call_running(&channel),
                state.me().to_owned(),
            )
        };
        let voice = account.voice();
        let previous = {
            let mut inner = voice.inner.lock();
            inner.session = None;
            inner.server = None;
            inner.active.take()
        };
        if let Some(previous) = previous {
            previous.shut();
            if previous.guild != guild {
                account.send(commands::voice_state(
                    previous.guild.as_deref(),
                    None,
                    false,
                    false,
                ));
            }
        }
        account.send(commands::voice_state(
            guild.as_deref(),
            Some(&channel),
            muted,
            deafened,
        ));
        if private && !running {
            account
                .rest()
                .post(format!("/channels/{channel}/call/ring"))
                .json(json!({ "recipients": null }))
                .done()
                .await?;
        }
        let answered =
            tokio::time::timeout(ANSWER_WITHIN, voice.answered(guild.as_deref(), &channel)).await;
        let Ok((voice_session, server, endpoint)) = answered else {
            account.send(commands::voice_state(guild.as_deref(), None, false, false));
            return Err(Error::Voice(
                "Discord did not answer the voice request in time".into(),
            ));
        };
        let host = snowflake::value(guild.as_deref().unwrap_or(&channel));
        let user = snowflake::id::<UserId>(&me);
        let (sink, events) = VoiceSink::new();
        let voice_channel: ChannelId = snowflake::id(&channel);
        let target = Endpoint {
            endpoint,
            token: server.token,
            server: host,
            channel: voice_channel,
            group: voice_channel.get(),
            user,
            session: voice_session.session.clone(),
        };
        let transport = match Transport::open(target, Role::Voice, sink.clone()).await {
            Ok(transport) => transport,
            Err(error) => {
                account.send(commands::voice_state(guild.as_deref(), None, false, false));
                return Err(error.into());
            }
        };
        let active = Arc::new(Active {
            guild,
            server: host,
            user,
            channel: Mutex::new(channel.clone()),
            session: Mutex::new(voice_session.session),
            sink,
            transport: Mutex::new(Some(Arc::new(transport))),
            leaving: AtomicBool::new(false),
            runtime: account.runtime().clone(),
        });
        voice.inner.lock().active = Some(Arc::clone(&active));
        Ok(Call {
            channel: snowflake::id(&channel),
            line: Line {
                active,
                account: Arc::downgrade(&account),
            },
            events,
        })
    }

    async fn answered(&self, guild: Option<&str>, channel: &str) -> (VoiceSession, Server, String) {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let inner = self.inner.lock();
                let voice_session = inner.session.clone().filter(|session| {
                    session.guild.as_deref() == guild && session.channel.as_deref() == Some(channel)
                });
                let server = inner.server.clone().filter(|server| match guild {
                    Some(guild) => server.guild.as_deref() == Some(guild),
                    None => {
                        server.guild.is_none()
                            && server
                                .channel
                                .as_deref()
                                .is_none_or(|known| known == channel)
                    }
                });
                if let (Some(voice_session), Some(server)) = (voice_session, server)
                    && let Some(endpoint) = server.endpoint.clone()
                {
                    return (voice_session, server, endpoint);
                }
            }
            notified.await;
        }
    }
}
