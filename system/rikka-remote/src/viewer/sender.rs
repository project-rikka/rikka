use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use rikka_rate_control::{Congestion, Limits, Pacer, Sent};
use rikka_remote_link::{Packetizer, acknowledgement, stamp};
use rikka_remote_wire::{
    AudioPacket, Feedback, ServerMessage, Stats, encode_padding, encode_pointer,
};

use crate::media::{Backlog, OutgoingFrame, SoundPacket, VideoCommand, VideoControl};
use crate::viewer::control::ControlWriter;
use crate::viewer::recovery::Recovery;

const PACING_FACTOR: f64 = 2.5;
const SOUND_BPS: u64 = 150_000;
const LOWEST_VIDEO_BPS: u64 = 300_000;
const LOWEST_BPS: u64 = 500_000;
const START_BPS: u64 = 8_000_000;
const RETARGET: f64 = 0.05;
const BASE_PROTECTION: f64 = 0.03;
const LOSS_PROTECTION: f64 = 3.0;
const MOST_PROTECTION: f64 = 0.5;
const INTRA_PROTECTION: f64 = 0.2;
const REDUNDANT_SOUND_LOSS: f64 = 0.005;
const FALLBACK_DATAGRAM: usize = 1200;
const STATS_EVERY: Duration = Duration::from_secs(1);
const PROBE_EVERY: Duration = Duration::from_millis(100);

#[derive(Debug, Default)]
struct Tally {
    frames: u32,
    encoding: Duration,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Continue,
    Lost,
}

pub(crate) struct Sender {
    connection: quinn::Connection,
    congestion: Congestion,
    pacer: Pacer<Vec<u8>>,
    packetizer: Packetizer,
    recovery: Recovery,
    backlog: Arc<Backlog>,
    video: VideoControl,
    writer: ControlWriter,
    seq: u32,
    video_bitrate: u64,
    acked_bps: Option<u64>,
    protection: f64,
    previous_sound: Option<Vec<u8>>,
    tally: Tally,
    next_stats: Instant,
    next_probe: Instant,
    shards: Vec<Vec<u8>>,
}

impl Sender {
    pub(crate) fn new(
        connection: quinn::Connection,
        highest_bps: u64,
        backlog: Arc<Backlog>,
        video: VideoControl,
        writer: ControlWriter,
    ) -> Sender {
        let now = Instant::now();
        let congestion = Congestion::new(limits(highest_bps));
        let target = congestion.target_bps();
        Sender {
            connection,
            pacer: Pacer::new((target as f64 * PACING_FACTOR) as u64),
            congestion,
            packetizer: Packetizer::new(),
            recovery: Recovery::default(),
            backlog,
            video,
            writer,
            seq: 0,
            video_bitrate: Self::start_bitrate(highest_bps),
            acked_bps: None,
            protection: BASE_PROTECTION,
            previous_sound: None,
            tally: Tally::default(),
            next_stats: now + STATS_EVERY,
            next_probe: now,
            shards: Vec::new(),
        }
    }

    pub(crate) fn start_bitrate(highest_bps: u64) -> u64 {
        video_share(START_BPS.min(highest_bps), BASE_PROTECTION)
    }

    pub(crate) fn video_bitrate(&self) -> u64 {
        self.video_bitrate
    }

    pub(crate) fn set_highest(&mut self, highest_bps: u64) {
        self.congestion.set_limits(limits(highest_bps));
        self.retarget(true);
    }

    pub(crate) fn begin_stream(&mut self, stream: u16) {
        self.recovery.begin_stream(stream);
    }

    pub(crate) fn frame(&mut self, outgoing: OutgoingFrame, now: Instant) {
        if let Some(abandoned) = outgoing.abandons {
            let withdrawn = self.pacer.withdraw(abandoned);
            tracing::trace!(frame = abandoned, withdrawn, "abandoned a refinement");
            self.recovery.abandoned(abandoned);
        }
        let frame = &outgoing.frame;
        let protection = match frame.flags.intra {
            true => self.protection.max(INTRA_PROTECTION),
            false => self.protection,
        };
        let datagram = self
            .connection
            .max_datagram_size()
            .unwrap_or(FALLBACK_DATAGRAM);
        self.shards.clear();
        if let Err(error) = self
            .packetizer
            .packetize(frame, datagram, protection, &mut self.shards)
        {
            tracing::warn!("cannot protect frame {}: {error}", frame.frame);
            return;
        }
        for shard in self.shards.drain(..) {
            self.pacer.push_video(frame.frame, shard);
        }
        if frame.flags.refinement {
            self.backlog.set_refinement(Some(frame.frame));
        }
        self.recovery.sent(frame.frame, outgoing.captured.max(now));
        self.tally.frames += 1;
        self.tally.encoding += outgoing.encode;
    }

    pub(crate) fn sound(&mut self, packet: SoundPacket) {
        let redundant = self.congestion.loss() > REDUNDANT_SOUND_LOSS;
        let mut datagram = Vec::with_capacity(packet.data.len() * 2 + 16);
        AudioPacket {
            seq: 0,
            timestamp: packet.samples as u32,
            primary: &packet.data,
            previous: self.previous_sound.as_deref().filter(|_| redundant),
        }
        .encode(&mut datagram);
        self.previous_sound = Some(packet.data);
        self.pacer.push_urgent(datagram);
    }

    pub(crate) fn pointer(&mut self, x: i32, y: i32) {
        let mut datagram = Vec::with_capacity(16);
        encode_pointer(0, x, y, &mut datagram);
        self.pacer.push_urgent(datagram);
    }

    pub(crate) fn feedback(&mut self, feedback: &Feedback, now: Instant) {
        let update =
            self.congestion
                .feedback(acknowledgement(feedback), now, self.connection.rtt());
        tracing::trace!(?update, "feedback");
        if update.acked_bps.is_some() {
            self.acked_bps = update.acked_bps;
        }
        self.retarget(false);
        if let Some(good) = self.recovery.feedback(feedback) {
            tracing::debug!(?good, lost = ?feedback.lost, "the viewer lost frames; recovering");
            self.video.send_or_log(VideoCommand::Recover(good));
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        if let Some(good) = self.recovery.stalled(now, self.connection.rtt()) {
            tracing::debug!(?good, "the viewer stopped confirming frames; recovering");
            self.video.send_or_log(VideoCommand::Recover(good));
        }
        if now >= self.next_probe {
            self.next_probe = now + PROBE_EVERY;
            if !self.pacer.is_probing() {
                let datagram = self
                    .connection
                    .max_datagram_size()
                    .unwrap_or(FALLBACK_DATAGRAM);
                if let Some(cluster) = self.congestion.probe(now, datagram) {
                    tracing::debug!(rate = cluster.rate_bps, "probing the path");
                    let mut padding = Vec::with_capacity(datagram);
                    encode_padding(0, datagram, &mut padding);
                    self.pacer.probe(cluster, padding, now);
                }
            }
        }
        if now >= self.next_stats {
            self.next_stats = now + STATS_EVERY;
            self.send_stats();
        }
    }

    pub(crate) fn pump(&mut self, now: Instant) -> Step {
        while let Some(outgoing) = self.pacer.pop(now) {
            let mut datagram = outgoing.packet;
            stamp(&mut datagram, self.seq);
            let bytes = datagram.len();
            match self.connection.send_datagram(Bytes::from(datagram)) {
                Ok(()) => {
                    self.congestion.sent(Sent {
                        seq: self.seq,
                        at: now,
                        bytes,
                        probe: outgoing.probe,
                    });
                    self.seq = self.seq.wrapping_add(1);
                }
                Err(quinn::SendDatagramError::ConnectionLost(_)) => return Step::Lost,
                Err(error) => tracing::debug!(bytes, "datagram refused: {error}"),
            }
        }
        if let Some(refinement) = self.backlog.refinement()
            && !self.pacer.holds_frame(refinement)
        {
            self.backlog.set_refinement(None);
        }
        let queued = self.pacer.queued_video();
        self.backlog.set_queued(queued);
        if self.backlog.drained(queued) {
            self.video.send_or_log(VideoCommand::Drained);
        }
        Step::Continue
    }

    pub(crate) fn wake(&self, now: Instant) -> Instant {
        let stall = self.recovery.next_check(self.connection.rtt());
        [
            self.pacer.wake_at(now),
            Some(self.next_stats),
            Some(self.next_probe),
            stall,
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(self.next_stats)
    }

    fn retarget(&mut self, force: bool) {
        let target = self.congestion.target_bps();
        self.pacer.set_rate((target as f64 * PACING_FACTOR) as u64);
        let loss = self.congestion.loss();
        self.protection = (BASE_PROTECTION + loss * LOSS_PROTECTION).min(MOST_PROTECTION);
        let video = video_share(target, self.protection);
        let moved =
            (video as f64 - self.video_bitrate as f64).abs() / self.video_bitrate.max(1) as f64;
        if force || moved >= RETARGET {
            self.video_bitrate = video;
            self.video.send_or_log(VideoCommand::Bitrate(video));
        }
    }

    fn send_stats(&mut self) {
        let frames = self.tally.frames.max(1);
        let stats = Stats {
            target_bps: self.congestion.target_bps(),
            estimate_bps: self.acked_bps.unwrap_or_default(),
            frame_rate_mhz: self.tally.frames.saturating_mul(1000),
            encode_us: (self.tally.encoding.as_micros() / u128::from(frames)) as u32,
            protection_permille: (self.protection * 1000.0) as u16,
            loss_permille: (self.congestion.loss() * 1000.0) as u16,
            rtt_us: self.connection.rtt().as_micros() as u32,
        };
        self.tally = Tally::default();
        self.writer.send(ServerMessage::Stats(stats));
    }
}

fn limits(highest_bps: u64) -> Limits {
    Limits {
        lowest_bps: LOWEST_BPS,
        start_bps: START_BPS.min(highest_bps),
        highest_bps,
    }
}

fn video_share(target: u64, protection: f64) -> u64 {
    ((target.saturating_sub(SOUND_BPS) as f64 / (1.0 + protection)) as u64).max(LOWEST_VIDEO_BPS)
}

trait SendOrLog {
    fn send_or_log(&self, command: VideoCommand);
}

impl SendOrLog for VideoControl {
    fn send_or_log(&self, command: VideoCommand) {
        if let Err(error) = self.send(command) {
            tracing::debug!("the picture thread is gone: {error}");
        }
    }
}
