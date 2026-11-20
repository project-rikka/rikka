mod broadcast;
mod heartbeat;
mod link;
mod media;
mod watch;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use discord_core::{Privacy, UserId, VideoCodec, VoiceEvent, VoicePacket, VoiceSink};
use discord_rtp::{Cipher, Feedback, Header, Mode, Payload, SENT_CODEC, sender_report};
use futures_channel::oneshot;
use rikka_net::Client;
use tokio::time::{Instant, Interval, MissedTickBehavior};

use self::broadcast::{Broadcast, BroadcastSignal};
use self::heartbeat::Heartbeat;
use self::link::{Event, Link};
use self::media::{Arrival, Media};
use self::watch::Watching;
use crate::dave::{self, Dave, Input, Reply};
use crate::endpoint::{Endpoint, Role};
use crate::error::{Error, Result};
use crate::gateway::{self, Closing, Description, Inbound, Ready};
use crate::outbox::{Outbox, Outgoing};
use crate::roster::Roster;

pub(crate) type OpenReply = oneshot::Sender<Result<()>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const OPEN_TIMEOUT: Duration = Duration::from_secs(20);
const KEEPALIVE: Duration = Duration::from_secs(5);
const REPORT_EVERY: Duration = Duration::from_secs(1);
const FEEDBACK_EVERY: Duration = Duration::from_millis(20);
const RESUME_ATTEMPTS: u32 = 5;
const RESUME_BACKOFF: Duration = Duration::from_secs(1);
const ASSUMED_ROUND_TRIP: Duration = Duration::from_millis(100);
const DATAGRAM: usize = 2048;

enum Step {
    Continue,
    Live,
    Finished(Option<Error>),
}

pub(crate) fn run(
    endpoint: Endpoint,
    role: Role,
    sink: VoiceSink,
    outbox: Arc<Outbox>,
    open_reply: OpenReply,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = open_reply.send(Err(Error::unreachable(error)));
            return;
        }
    };
    runtime.block_on(serve(endpoint, role, sink, outbox, open_reply));
}

async fn serve(
    endpoint: Endpoint,
    role: Role,
    sink: VoiceSink,
    outbox: Arc<Outbox>,
    open_reply: OpenReply,
) {
    let mut open_reply = Some(open_reply);
    let connected = Session::connect(endpoint, role, sink.clone(), Arc::clone(&outbox)).await;
    let mut session = match connected {
        Ok(session) => session,
        Err(error) => {
            if let Some(open_reply) = open_reply.take() {
                let _ = open_reply.send(Err(error));
            }
            outbox.close();
            return;
        }
    };
    let error = session.drive(&mut open_reply).await;
    session.link.close().await;
    outbox.close();
    if session.live {
        sink.send(VoiceEvent::Ended(error.map(Into::into)));
    } else if let (Some(open_reply), Some(error)) = (open_reply, error) {
        let _ = open_reply.send(Err(error));
    }
}

struct Session {
    endpoint: Endpoint,
    role: Role,
    url: String,
    user: u64,
    sink: VoiceSink,
    outbox: Arc<Outbox>,
    client: Client,
    link: Link,
    seq: i64,
    heartbeat: Option<Heartbeat>,
    round_trip: Option<Duration>,
    media: Option<Media>,
    dave: Option<Dave>,
    early: Vec<Input>,
    roster: Roster,
    broadcast: Option<Broadcast>,
    watching: Watching,
    privacy: Option<Privacy>,
    replies: Vec<Reply>,
    live: bool,
    speaking: bool,
    announced: bool,
    resumes: u32,
}

impl Session {
    async fn connect(
        endpoint: Endpoint,
        role: Role,
        sink: VoiceSink,
        outbox: Arc<Outbox>,
    ) -> Result<Session> {
        let url = endpoint.url()?;
        let user = endpoint.user.get();
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(Error::unreachable)?;
        let mut link = Link::connect(&client, &url).await?;
        link.send(gateway::identify(&endpoint, role, dave::MAX_VERSION))
            .await;
        Ok(Session {
            endpoint,
            role,
            url,
            user,
            sink,
            outbox,
            client,
            link,
            seq: -1,
            heartbeat: None,
            round_trip: None,
            media: None,
            dave: None,
            early: Vec::new(),
            roster: Roster::default(),
            broadcast: None,
            watching: Watching::default(),
            privacy: None,
            replies: Vec::new(),
            live: false,
            speaking: false,
            announced: false,
            resumes: 0,
        })
    }

    async fn drive(&mut self, open_reply: &mut Option<OpenReply>) -> Option<Error> {
        let deadline = Instant::now() + OPEN_TIMEOUT;
        let mut keepalive = ticker(KEEPALIVE);
        let mut feeding_back = ticker(FEEDBACK_EVERY);
        let mut reporting = ticker(REPORT_EVERY);
        let mut buffer = vec![0; DATAGRAM];
        loop {
            if self.outbox.is_closed() {
                return None;
            }
            self.flush().await;
            let video = self.role.has_video() && self.media.is_some();
            let pace_at = self
                .broadcast
                .as_ref()
                .and_then(|broadcast| broadcast.wake_at(std::time::Instant::now()));
            let watching = self.media.is_some() && !self.watching.is_empty();
            let step = tokio::select! {
                event = self.link.next() => self.on_event(event).await,
                received = receive(self.media.as_ref(), &mut buffer) => match received {
                    Ok(length) => self.on_datagram(&buffer[..length]).await,
                    Err(error) => {
                        tracing::debug!(%error, "failed to receive voice datagram");
                        Step::Continue
                    }
                },
                () = beat_due(self.heartbeat.as_ref()) => self.beat().await,
                _ = keepalive.tick(), if self.media.is_some() => self.keepalive().await,
                () = pace_due(pace_at) => self.pace().await,
                _ = feeding_back.tick(), if watching => self.feed_back().await,
                _ = reporting.tick(), if video => self.report().await,
                () = self.outbox.notified() => Step::Continue,
                () = tokio::time::sleep_until(deadline), if !self.live => {
                    Step::Finished(Some(Error::Timeout))
                }
                () = abandoned(open_reply) => Step::Finished(None),
            };
            match step {
                Step::Continue => {}
                Step::Live => {
                    let sender = open_reply.take()?;
                    if sender.send(Ok(())).is_err() {
                        self.live = false;
                        return None;
                    }
                }
                Step::Finished(error) => return error,
            }
        }
    }

    async fn on_event(&mut self, event: Event) -> Step {
        match event {
            Event::Received(received) => {
                if let Some(seq) = received.seq {
                    self.seq = seq;
                }
                self.on_inbound(received.inbound).await
            }
            Event::Closed { code, reason } => self.on_closed(code, &reason).await,
        }
    }

    async fn on_inbound(&mut self, inbound: Inbound) -> Step {
        match inbound {
            Inbound::Hello { interval } => {
                self.heartbeat = Some(Heartbeat::new(interval, Instant::now()));
            }
            Inbound::Ready(ready) => return self.on_ready(ready).await,
            Inbound::Described(description) => return self.on_described(description).await,
            Inbound::Updated { keyframes } => {
                if self.role == Role::Broadcast
                    && let Some(every) = keyframes
                {
                    tracing::debug!(?every, "stream server changed keyframe interval");
                    self.sink.send(VoiceEvent::KeyframeEvery(every));
                }
            }
            Inbound::Speaking { user, ssrc } => self.source(ssrc, user),
            Inbound::Video(video) => {
                self.source(video.audio, video.user);
                if self.role == Role::Watch {
                    let events = self.watching.announce(&video);
                    let announced = !events.is_empty();
                    self.emit(events);
                    if announced {
                        self.want().await;
                    }
                }
            }
            Inbound::Wanted(wants) => {
                if let Some(broadcast) = self.broadcast.as_mut() {
                    let quality = wants.quality_for(broadcast.ssrc()).unwrap_or(100);
                    let signals = broadcast.want(quality);
                    self.forward_signals(signals);
                }
            }
            Inbound::Ack { nonce } => {
                let latency = self
                    .heartbeat
                    .as_mut()
                    .and_then(|heartbeat| heartbeat.acknowledged(nonce, Instant::now()));
                if let Some(latency) = latency {
                    self.round_trip = Some(latency);
                    if self.live {
                        self.sink.send(VoiceEvent::Latency(latency));
                    }
                }
            }
            Inbound::Resumed => return self.on_resumed().await,
            Inbound::Joined(users) => {
                for user in users {
                    self.roster.join(user);
                }
            }
            Inbound::Left(user) => {
                for source in self.roster.leave(user) {
                    if self.live {
                        self.sink.send(VoiceEvent::SourceGone { source });
                    }
                }
                let events = self.watching.left(user);
                self.emit(events);
            }
            Inbound::Dave(input) => {
                let Some(dave) = self.dave.as_mut() else {
                    self.early.push(input);
                    return Step::Continue;
                };
                let members = self.roster.members(self.user);
                if let Err(error) = dave.handle(input, &members, &mut self.replies) {
                    return Step::Finished(Some(error));
                }
                self.answer().await;
            }
            Inbound::Ignored(op) => tracing::trace!(op, "ignored voice gateway message"),
        }
        Step::Continue
    }

    async fn on_ready(&mut self, ready: Ready) -> Step {
        let Some(mode) = Mode::choose(&ready.modes) else {
            return Step::Finished(Some(Error::NoMode));
        };
        let (media, discovered) = match Media::connect(&ready.ip, ready.port, ready.ssrc).await {
            Ok(connected) => connected,
            Err(error) => return Step::Finished(Some(error)),
        };
        self.media = Some(media);
        if self.role == Role::Broadcast {
            let Some(stream) = ready.streams.first() else {
                return Step::Finished(Some(Error::Unreadable(
                    "a stream connection without a video stream".to_owned(),
                )));
            };
            let repair = stream.rtx_ssrc.unwrap_or(stream.ssrc.wrapping_add(1));
            self.broadcast = Some(Broadcast::new(stream.ssrc, repair));
        }
        self.link
            .send(gateway::select_protocol(
                &discovered.address,
                discovered.port,
                mode,
                self.role,
            ))
            .await;
        Step::Continue
    }

    async fn on_described(&mut self, description: Description) -> Step {
        let Some(mode) = Mode::named(&description.mode) else {
            return Step::Finished(Some(Error::NoMode));
        };
        let Some(media) = self.media.as_mut() else {
            return Step::Finished(Some(Error::Unreadable(
                "a session description before ready".to_owned(),
            )));
        };
        media.secure(Cipher::new(mode, &description.key));
        tracing::debug!(
            role = ?self.role,
            codec = ?description.video_codec,
            dave = description.dave,
            keyframes = ?description.keyframes,
            "voice session description received"
        );
        if self.role == Role::Broadcast
            && let Some(chosen) = description.video_codec.as_deref()
            && VideoCodec::named(chosen) != Some(SENT_CODEC)
        {
            tracing::warn!(chosen, "stream server chose unsupported codec");
        }
        if self.role == Role::Broadcast
            && let Some(every) = description.keyframes
        {
            self.sink.send(VoiceEvent::KeyframeEvery(every));
        }
        if self.dave.is_none() {
            match self.start_dave(description.dave) {
                Ok(dave) => self.dave = Some(dave),
                Err(error) => return Step::Finished(Some(error)),
            }
            self.answer().await;
        }
        if self.live {
            return Step::Continue;
        }
        self.live = true;
        let privacy = self.current_privacy();
        self.privacy = Some(privacy.clone());
        self.sink.send(VoiceEvent::Connected { encrypted: privacy });
        for (source, user) in self.roster.sources() {
            self.sink.send(VoiceEvent::Source { source, user });
        }
        for (source, user) in self.watching.known() {
            self.sink.send(VoiceEvent::VideoSource { source, user });
        }
        if self.role == Role::Watch {
            self.want().await;
        }
        Step::Live
    }

    async fn on_resumed(&mut self) -> Step {
        self.resumes = 0;
        let Some(media) = self.media.as_ref() else {
            return Step::Continue;
        };
        let ssrc = media.ssrc();
        if self.announced {
            self.link
                .send(gateway::speaking(true, ssrc, self.role))
                .await;
        }
        if let Some(broadcast) = self.broadcast.as_ref()
            && let Some(shape) = broadcast.shape()
        {
            let sending = broadcast.sending(ssrc);
            self.link.send(gateway::video(&sending, Some(shape))).await;
        }
        if self.role == Role::Watch {
            self.want().await;
        }
        Step::Continue
    }

    fn start_dave(&mut self, version: u16) -> Result<Dave> {
        let mut dave = Dave::new(self.user, self.endpoint.group, version, &mut self.replies)?;
        let members = self.roster.members(self.user);
        for input in std::mem::take(&mut self.early) {
            dave.handle(input, &members, &mut self.replies)?;
        }
        Ok(dave)
    }

    async fn on_datagram(&mut self, datagram: &[u8]) -> Step {
        let arrived = std::time::Instant::now();
        let arrival = self.media.as_ref().and_then(|media| media.open(datagram));
        if let Some(broadcast) = self.broadcast.as_mut() {
            broadcast.arrived(arrival.is_some());
        }
        let Some(arrival) = arrival else {
            return Step::Continue;
        };
        match arrival {
            Arrival::Control(feedback) => {
                self.on_feedback(&feedback);
                Step::Continue
            }
            Arrival::Media(media) => match Payload::from_type(media.header.payload_type) {
                Some(Payload::Opus) => self.on_voice(media.header, media.payload, arrived).await,
                Some(kind) if self.role == Role::Watch => {
                    let events = self.watching.packet(
                        media.header,
                        kind,
                        media.payload,
                        self.dave.as_mut(),
                        &mut self.replies,
                        Instant::now().into_std(),
                    );
                    match events {
                        Ok(events) => {
                            if !self.replies.is_empty() {
                                self.answer().await;
                            }
                            self.emit(events);
                            Step::Continue
                        }
                        Err(error) => Step::Finished(Some(error)),
                    }
                }
                _ => Step::Continue,
            },
        }
    }

    async fn on_voice(
        &mut self,
        header: Header,
        payload: Vec<u8>,
        arrived: std::time::Instant,
    ) -> Step {
        let opus = match self.dave.as_mut() {
            None => payload,
            Some(dave) => {
                let sender = self.roster.user(header.ssrc).map(UserId::get);
                match dave.decrypt(sender, &payload, &mut self.replies) {
                    Ok(Some(plain)) => plain.into_owned(),
                    Ok(None) => {
                        self.answer().await;
                        return Step::Continue;
                    }
                    Err(error) => return Step::Finished(Some(error)),
                }
            }
        };
        if self.live {
            self.sink.send(VoiceEvent::Packet(VoicePacket {
                source: header.ssrc,
                timestamp: header.timestamp,
                arrived,
                opus,
            }));
        }
        Step::Continue
    }

    fn on_feedback(&mut self, feedback: &[Feedback]) {
        let now = Instant::now().into_std();
        let round_trip = self.round_trip.unwrap_or(ASSUMED_ROUND_TRIP);
        for item in feedback {
            self.watching.on_feedback(item, now);
            let Some(broadcast) = self.broadcast.as_mut() else {
                continue;
            };
            let signals = broadcast.on_feedback(item, now, round_trip);
            self.forward_signals(signals);
        }
    }

    fn forward_signals(&self, signals: Vec<BroadcastSignal>) {
        if !self.live {
            return;
        }
        for signal in signals {
            self.sink.send(match signal {
                BroadcastSignal::Wanted(quality) => VoiceEvent::Wanted(quality),
                BroadcastSignal::KeyframeWanted => VoiceEvent::KeyframeWanted,
                BroadcastSignal::Bandwidth(bitrate) => VoiceEvent::Bandwidth(bitrate),
            });
        }
    }

    fn emit(&self, events: Vec<VoiceEvent>) {
        if !self.live {
            return;
        }
        for event in events {
            self.sink.send(event);
        }
    }

    async fn want(&mut self) {
        let sources = self.watching.sources();
        self.link.send(gateway::wants(&sources)).await;
    }

    async fn pace(&mut self) -> Step {
        let now = Instant::now().into_std();
        let Some(media) = self.media.as_mut() else {
            return Step::Continue;
        };
        if let Some(broadcast) = self.broadcast.as_mut() {
            for packet in broadcast.release(media, now) {
                media.transmit(&packet).await;
            }
        }
        Step::Continue
    }

    async fn feed_back(&mut self) -> Step {
        let Some(me) = self.media.as_ref().map(Media::ssrc) else {
            return Step::Continue;
        };
        let round_trip = self.round_trip.unwrap_or(ASSUMED_ROUND_TRIP);
        let feedback = self
            .watching
            .feedback(me, Instant::now().into_std(), round_trip);
        self.control(feedback).await;
        Step::Continue
    }

    async fn report(&mut self) -> Step {
        let now = Instant::now().into_std();
        let Some(media) = self.media.as_ref() else {
            return Step::Continue;
        };
        let me = media.ssrc();
        let mut reports = Vec::new();
        if let Some(broadcast) = self.broadcast.as_mut() {
            broadcast.log_tally();
            if let Some(sent) = broadcast.report() {
                reports.push(sender_report(sent));
            }
        }
        if let Some(sent) = media.audio_report() {
            reports.push(sender_report(sent));
        }
        if let Some(received) = self.watching.report(me, now) {
            reports.push(received);
        }
        self.control(reports).await;
        Step::Continue
    }

    async fn control(&mut self, packets: Vec<Vec<u8>>) {
        let Some(media) = self.media.as_mut() else {
            return;
        };
        for packet in packets {
            if let Some(sealed) = media.seal_control(&packet) {
                media.transmit(&sealed).await;
            }
        }
    }

    async fn on_closed(&mut self, code: Option<u16>, reason: &str) -> Step {
        let closing = gateway::closing(code, reason);
        if !self.live {
            return Step::Finished(Some(match closing {
                Closing::Failed(error) => error,
                Closing::Resume | Closing::Finished => Error::Unreachable(format!(
                    "the voice server closed the connection ({}) {reason}",
                    code.map_or_else(|| "without a code".to_owned(), |code| code.to_string())
                )),
            }));
        }
        match closing {
            Closing::Finished => Step::Finished(None),
            Closing::Failed(error) => Step::Finished(Some(error)),
            Closing::Resume => self.resume(reason).await,
        }
    }

    async fn resume(&mut self, reason: &str) -> Step {
        tracing::debug!(reason, "voice gateway connection lost, resuming");
        self.heartbeat = None;
        let mut failure = Error::Unreachable(reason.to_owned());
        while self.resumes < RESUME_ATTEMPTS {
            self.resumes += 1;
            if !self.pause(RESUME_BACKOFF * self.resumes).await {
                return Step::Finished(None);
            }
            match Link::connect(&self.client, &self.url).await {
                Ok(link) => {
                    self.link = link;
                    self.link
                        .send(gateway::resume(&self.endpoint, self.seq))
                        .await;
                    return Step::Continue;
                }
                Err(error) => failure = error,
            }
        }
        Step::Finished(Some(failure))
    }

    async fn pause(&self, delay: Duration) -> bool {
        let until = Instant::now() + delay;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(until) => return !self.outbox.is_closed(),
                () = self.outbox.notified() => {
                    if self.outbox.is_closed() {
                        return false;
                    }
                }
            }
        }
    }

    async fn beat(&mut self) -> Step {
        let Some(heartbeat) = self.heartbeat.as_mut() else {
            return Step::Continue;
        };
        match heartbeat.beat(Instant::now()) {
            Some(nonce) => {
                self.link.send(gateway::heartbeat(nonce, self.seq)).await;
                Step::Continue
            }
            None => self.on_closed(None, "the heartbeat went unanswered").await,
        }
    }

    async fn keepalive(&mut self) -> Step {
        if let Some(media) = self.media.as_mut() {
            media.keepalive().await;
        }
        Step::Continue
    }

    async fn flush(&mut self) {
        if self.outbox.overflowed() {
            self.forward_signals(vec![BroadcastSignal::KeyframeWanted]);
        }
        for outgoing in self.outbox.take() {
            let Some(media) = self.media.as_mut() else {
                continue;
            };
            match outgoing {
                Outgoing::Speaking(on) => {
                    self.speaking = on;
                    if on {
                        media.talkspurt();
                    }
                    self.announce().await;
                }
                Outgoing::Frame(opus) => {
                    let frame = match self.dave.as_mut() {
                        Some(dave) => dave.encrypt(&opus),
                        None => Some(Cow::Borrowed(&opus[..])),
                    };
                    if let Some(frame) = frame {
                        media.send(&frame).await;
                    }
                }
                Outgoing::Video(frame) => {
                    if let Some(broadcast) = self.broadcast.as_mut() {
                        broadcast.frame(media.is_secure(), self.dave.as_mut(), &frame);
                    }
                }
                Outgoing::Refresh(source) => self.watching.broke(source),
                Outgoing::Shape(shape) => {
                    let ssrc = media.ssrc();
                    let Some(broadcast) = self.broadcast.as_mut() else {
                        continue;
                    };
                    if broadcast.shape() == shape {
                        continue;
                    }
                    tracing::debug!(?shape, "announcing stream video");
                    let signals = broadcast.reshape(shape);
                    let sending = broadcast.sending(ssrc);
                    self.link.send(gateway::video(&sending, shape)).await;
                    self.forward_signals(signals);
                    self.announce().await;
                }
            }
        }
    }

    async fn announce(&mut self) {
        let Some(ssrc) = self.media.as_ref().map(Media::ssrc) else {
            return;
        };
        let showing = self
            .broadcast
            .as_ref()
            .is_some_and(|broadcast| broadcast.shape().is_some());
        let speaking = self.speaking || showing;
        if speaking == self.announced {
            return;
        }
        self.announced = speaking;
        self.link
            .send(gateway::speaking(speaking, ssrc, self.role))
            .await;
    }

    async fn answer(&mut self) {
        for reply in std::mem::take(&mut self.replies) {
            self.link.send(gateway::reply(reply)).await;
        }
        if !self.live {
            return;
        }
        let privacy = self.current_privacy();
        if self.privacy.as_ref() != Some(&privacy) {
            tracing::debug!(role = ?self.role, ?privacy, "media privacy changed");
            self.privacy = Some(privacy.clone());
            self.sink.send(VoiceEvent::Privacy(privacy));
        }
    }

    fn current_privacy(&self) -> Privacy {
        self.dave.as_ref().map_or(Privacy::Transport, Dave::privacy)
    }

    fn source(&mut self, ssrc: u32, user: UserId) {
        if self.roster.source(ssrc, user) && self.live {
            self.sink.send(VoiceEvent::Source { source: ssrc, user });
        }
    }
}

fn ticker(period: Duration) -> Interval {
    let mut interval = tokio::time::interval(period);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
}

async fn receive(media: Option<&Media>, buffer: &mut [u8]) -> std::io::Result<usize> {
    match media {
        Some(media) => media.receive(buffer).await,
        None => std::future::pending().await,
    }
}

async fn beat_due(heartbeat: Option<&Heartbeat>) {
    match heartbeat {
        Some(heartbeat) => tokio::time::sleep_until(heartbeat.due()).await,
        None => std::future::pending().await,
    }
}

async fn pace_due(at: Option<std::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(Instant::from_std(at)).await,
        None => std::future::pending().await,
    }
}

async fn abandoned(open_reply: &mut Option<OpenReply>) {
    match open_reply.as_mut() {
        Some(sender) => sender.cancellation().await,
        None => std::future::pending().await,
    }
}
