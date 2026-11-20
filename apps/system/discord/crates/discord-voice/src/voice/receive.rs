use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use discord_core::{UserId, VoiceEvent, VoiceEvents, VoicePacket};
use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt as _;
use rikka_apm::Reference;
use rikka_playout::QueueTarget;
use rikka_worker::EventFd;

use crate::input::STEP;
use crate::io::{Devices, Playback};
use crate::output::{Mixer, OUTPUT_CHANNELS, Stream};
use crate::settings::VoiceSettings;
use crate::voice::outbox::Outbox;
use crate::voice::shared::{Shared, UserMix};
use crate::voice::update::VoiceUpdate;

const MAX_FRAMES_PER_PULL: u32 = 8;
const MAX_CLOCK_LAG: Duration = Duration::from_millis(100);

struct StreamSoundSource {
    packets: UnboundedReceiver<VoicePacket>,
    streams: HashMap<u32, Stream>,
    gain: f32,
}

pub(crate) struct Receiving {
    events: VoiceEvents,
    stream_sources: HashMap<u64, StreamSoundSource>,
    stream_volume_revision: u64,
    shared: Arc<Shared>,
    outbox: Outbox,
    playback: Playback,
    queue_target: QueueTarget,
    reference: Reference,
    streams: HashMap<u32, Stream>,
    user_mix: HashMap<UserId, UserMix>,
    user_mix_revision: u64,
    revision: u64,
    volume: f32,
    mixer: Mixer,
    origin: Instant,
    clock: Instant,
    deafened: bool,
}

impl Receiving {
    pub(crate) fn new(
        events: VoiceEvents,
        shared: Arc<Shared>,
        outbox: Outbox,
        devices: Arc<dyn Devices>,
        reference: Reference,
        settings: &VoiceSettings,
    ) -> Receiving {
        devices.hotplug().watch(shared.receive_bell().clone());
        let origin = Instant::now();
        Receiving {
            events,
            stream_sources: HashMap::new(),
            stream_volume_revision: 0,
            shared,
            outbox,
            playback: Playback::new(devices, settings.output.clone()),
            queue_target: QueueTarget::new(),
            reference,
            streams: HashMap::new(),
            user_mix: HashMap::new(),
            user_mix_revision: 0,
            revision: 0,
            volume: settings.output_gain(),
            mixer: Mixer::new(),
            origin,
            clock: origin,
            deafened: false,
        }
    }

    pub(crate) fn run(mut self, bell: &EventFd) {
        let waker = Waker::from(Arc::new(self.shared.receive_bell().clone()));
        let mut heard = Context::from_waker(&waker);
        loop {
            bell.drain();
            if self.shared.stopped() {
                return;
            }
            if let Some(ended) = self.drain(&mut heard) {
                if !self.shared.stopped() {
                    self.shared.stop();
                    self.outbox.send(VoiceUpdate::Ended(ended));
                }
                return;
            }
            self.refresh();
            self.receive_stream_sound(&mut heard);
            let now = Instant::now();
            if !self.deafened && self.playback.poll() {
                self.outbox.send(VoiceUpdate::DeviceLost);
            }
            self.play(now);
            if let Err(error) = self.wait_for_next_pull(bell) {
                tracing::warn!(%error, "cannot wait for the voice output");
                return;
            }
        }
    }

    fn wait_for_next_pull(&mut self, bell: &EventFd) -> std::io::Result<()> {
        if !self.deafened
            && let Some(speaker) = self.playback.speaker()
        {
            let queued = speaker.queued();
            if queued < self.queue_target.target() || speaker.watch_queued_below(queued) {
                return Ok(());
            }
            return bell.wait_readable(speaker.as_fd(), None).map(drop);
        }
        let next_frame = self.clock.saturating_duration_since(Instant::now());
        bell.wait_timeout(next_frame)
    }

    fn drain(&mut self, heard: &mut Context<'_>) -> Option<Option<discord_core::Error>> {
        loop {
            match self.events.poll_next_unpin(heard) {
                Poll::Ready(Some(VoiceEvent::Ended(error))) => return Some(error),
                Poll::Ready(Some(event)) => self.handle(event),
                Poll::Pending => return None,
                Poll::Ready(None) => {
                    return Some(Some(discord_core::Error::Voice(
                        "voice connection closed".into(),
                    )));
                }
            }
        }
    }

    fn handle(&mut self, event: VoiceEvent) {
        match event {
            VoiceEvent::Connected { encrypted } => {
                self.outbox.send(VoiceUpdate::Connected(encrypted));
            }
            VoiceEvent::Privacy(privacy) => self.outbox.send(VoiceUpdate::Privacy(privacy)),
            VoiceEvent::Source { source, user } => {
                if let Some(stream) = source_in(&mut self.streams, source, self.origin) {
                    stream.set_user(user);
                }
            }
            VoiceEvent::SourceGone { source } => {
                let Some(stream) = self.streams.remove(&source) else {
                    return;
                };
                if let (true, Some(user)) = (stream.is_speaking(), stream.user()) {
                    self.outbox.send(VoiceUpdate::Speaking {
                        user: *user,
                        on: false,
                    });
                }
            }
            VoiceEvent::Packet(packet) => {
                if let Some(stream) = source_in(&mut self.streams, packet.source, self.origin) {
                    stream.push(packet);
                }
            }
            VoiceEvent::Latency(latency) => self.outbox.send(VoiceUpdate::Latency(latency)),
            VoiceEvent::Moved(channel) => self.outbox.send(VoiceUpdate::Moved(channel)),
            VoiceEvent::Speaking { .. }
            | VoiceEvent::VideoSource { .. }
            | VoiceEvent::VideoGone { .. }
            | VoiceEvent::Video(_)
            | VoiceEvent::Wanted(_)
            | VoiceEvent::KeyframeWanted
            | VoiceEvent::KeyframeEvery(_)
            | VoiceEvent::Bandwidth(_)
            | VoiceEvent::Viewers(_)
            | VoiceEvent::Paused(_)
            | VoiceEvent::Ended(_) => {}
        }
    }

    fn receive_stream_sound(&mut self, heard: &mut Context<'_>) {
        for (id, packets) in self.shared.attached().arrived() {
            self.stream_sources.insert(
                id,
                StreamSoundSource {
                    packets,
                    streams: HashMap::new(),
                    gain: 1.0,
                },
            );
        }
        if let Some((revision, volumes)) = self
            .shared
            .attached()
            .volumes_since(self.stream_volume_revision)
        {
            self.stream_volume_revision = revision;
            for (id, source) in &mut self.stream_sources {
                source.gain = volumes.get(id).copied().unwrap_or(1.0);
            }
        }
        let origin = self.origin;
        self.stream_sources.retain(|_, source| {
            loop {
                match source.packets.poll_next_unpin(heard) {
                    Poll::Ready(Some(packet)) => {
                        if let Some(stream) = source_in(&mut source.streams, packet.source, origin)
                        {
                            stream.push(packet);
                        }
                    }
                    Poll::Pending => break true,
                    Poll::Ready(None) => break false,
                }
            }
        });
    }

    fn refresh(&mut self) {
        if let Some((revision, settings)) = self.shared.settings_since(self.revision) {
            self.revision = revision;
            self.volume = settings.output_gain();
            self.playback.choose(settings.output);
            self.queue_target.reset();
        }
        if let Some((revision, user_mix)) = self.shared.user_mix_since(self.user_mix_revision) {
            self.user_mix_revision = revision;
            self.user_mix = user_mix;
        }
        let deafened = self.shared.deafened();
        if deafened != self.deafened {
            self.deafened = deafened;
            if deafened {
                self.playback.release();
            }
        }
    }

    fn play(&mut self, now: Instant) {
        let queued = if self.deafened {
            None
        } else {
            self.playback.queued()
        };
        if let Some(mut queued) = queued {
            self.queue_target
                .observe(now, queued, self.playback.underruns());
            let target = self.queue_target.target();
            let mut produced = 0;
            while queued < target && produced < MAX_FRAMES_PER_PULL {
                self.produce(now, true);
                self.queue_target.wrote(STEP);
                produced += 1;
                queued = self.playback.queued().unwrap_or(Duration::MAX);
            }
            self.clock = now;
            return;
        }
        self.queue_target.reset();
        if now.saturating_duration_since(self.clock) > MAX_CLOCK_LAG {
            self.clock = now;
        }
        while self.clock <= now {
            self.produce(now, false);
            self.clock += STEP;
        }
    }

    fn produce(&mut self, now: Instant, audible: bool) {
        self.mixer.begin();
        for stream in self.streams.values_mut() {
            let gain = stream
                .user()
                .and_then(|user| self.user_mix.get(user))
                .map_or(1.0, UserMix::gain);
            let chunk = stream.pull(now);
            let speaking = chunk.speaking;
            if let Some(audio) = chunk.audio {
                self.mixer.add(audio, gain);
            }
            if let (Some(on), Some(user)) = (speaking, stream.user()) {
                self.outbox.send(VoiceUpdate::Speaking { user: *user, on });
            }
        }
        for source in self.stream_sources.values_mut() {
            for stream in source.streams.values_mut() {
                if let Some(audio) = stream.pull(now).audio {
                    self.mixer.add(audio, source.gain);
                }
            }
        }
        let mixed = self.mixer.finish(self.volume);
        if audible {
            self.reference.render(mixed, usize::from(OUTPUT_CHANNELS));
            self.playback.write(mixed);
        }
    }
}

fn source_in(
    streams: &mut HashMap<u32, Stream>,
    source: u32,
    origin: Instant,
) -> Option<&mut Stream> {
    match streams.entry(source) {
        Entry::Occupied(known) => Some(known.into_mut()),
        Entry::Vacant(vacant) => match Stream::new(origin) {
            Ok(stream) => Some(vacant.insert(stream)),
            Err(error) => {
                tracing::warn!(%error, source, "failed to open voice decoder");
                None
            }
        },
    }
}
