use std::time::{Duration, Instant};

use discord_core::{EncodedVideo, VideoShape};
use discord_rtp::{
    Feedback, Plain, SENT_CODEC, SenderInfo, VideoSender, packetize_h264, prepare_h264,
};
use rikka_rate_control::{Limits, LossBased, Pacer};

use crate::dave::Dave;
use crate::gateway::Sending;
use crate::session::media::Media;

const PACING: f64 = 2.5;
const PICTURE_EVERY: Duration = Duration::from_millis(250);
const UNSHAPED_CEILING: u64 = 2_500_000;
const LOWEST_BPS: u64 = 300_000;
const NOTABLE: f64 = 0.05;
const LOSS_SCALE: f64 = 256.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BroadcastSignal {
    Wanted(u8),
    KeyframeWanted,
    Bandwidth(u32),
}

#[derive(Debug, Default)]
struct Tally {
    frames: u32,
    keyframes: u32,
    unwanted: u32,
    unsealed: u32,
    packets: u32,
    repairs: u32,
    pictures: u32,
    control: u32,
    datagrams: u32,
    unreadable: u32,
}

#[derive(Debug)]
pub(crate) struct Broadcast {
    sender: VideoSender,
    shape: Option<VideoShape>,
    requested_quality: u8,
    pacer: Pacer<Plain>,
    bandwidth: LossBased,
    reported: u64,
    frame: u32,
    last_picture_request: Option<Instant>,
    tally: Tally,
}

impl Broadcast {
    pub(crate) fn new(ssrc: u32, repair: u32) -> Broadcast {
        Broadcast {
            sender: VideoSender::new(ssrc, repair),
            shape: None,
            requested_quality: 100,
            pacer: Pacer::new(pacing(UNSHAPED_CEILING)),
            bandwidth: LossBased::new(limits(UNSHAPED_CEILING)),
            reported: UNSHAPED_CEILING,
            frame: 0,
            last_picture_request: None,
            tally: Tally::default(),
        }
    }

    pub(crate) fn log_tally(&mut self) {
        let tally = std::mem::take(&mut self.tally);
        tracing::debug!(
            ssrc = self.sender.ssrc(),
            requested_quality = self.requested_quality,
            shown = self.shape.is_some(),
            frames = tally.frames,
            keyframes = tally.keyframes,
            unwanted = tally.unwanted,
            unsealed = tally.unsealed,
            packets = tally.packets,
            repairs = tally.repairs,
            pictures = tally.pictures,
            control = tally.control,
            datagrams = tally.datagrams,
            unreadable = tally.unreadable,
            target = self.bandwidth.rate_bps(),
            "stream video in last second"
        );
    }

    pub(crate) fn sending(&self, audio: u32) -> Sending {
        Sending {
            audio,
            video: self.sender.ssrc(),
            repair: self.sender.repair_ssrc(),
        }
    }

    pub(crate) fn ssrc(&self) -> u32 {
        self.sender.ssrc()
    }

    pub(crate) fn shape(&self) -> Option<VideoShape> {
        self.shape
    }

    pub(crate) fn reshape(&mut self, shape: Option<VideoShape>) -> Vec<BroadcastSignal> {
        self.shape = shape;
        let Some(shape) = shape else {
            self.pacer.clear();
            return Vec::new();
        };
        let bitrate = u64::from(shape.bitrate);
        let at_ceiling = self.bandwidth.rate_bps() >= self.bandwidth.highest_bps();
        self.bandwidth.set_limits(limits(bitrate));
        if at_ceiling {
            self.bandwidth.raise_to(bitrate);
        }
        let target = self.bandwidth.rate_bps();
        self.pacer.set_rate(pacing(target));
        self.reported = target;
        vec![BroadcastSignal::Bandwidth(target as u32)]
    }

    pub(crate) fn want(&mut self, quality: u8) -> Vec<BroadcastSignal> {
        if quality == self.requested_quality {
            return Vec::new();
        }
        let resumed = self.requested_quality == 0 && quality > 0;
        self.requested_quality = quality;
        if quality == 0 {
            self.pacer.clear();
        }
        let mut signals = vec![BroadcastSignal::Wanted(quality)];
        if resumed {
            signals.push(BroadcastSignal::KeyframeWanted);
        }
        signals
    }

    pub(crate) fn frame(&mut self, secure: bool, dave: Option<&mut Dave>, frame: &EncodedVideo) {
        if self.requested_quality == 0 || self.shape.is_none() || !secure {
            self.tally.unwanted += 1;
            return;
        }
        self.tally.frames += 1;
        if frame.keyframe {
            self.tally.keyframes += 1;
        }
        let prepared = prepare_h264(&frame.data);
        let sealed = match dave {
            Some(dave) => dave.encrypt_video(SENT_CODEC, &prepared),
            None => Some(std::borrow::Cow::Borrowed(&prepared[..])),
        };
        let Some(sealed) = sealed else {
            self.tally.unsealed += 1;
            return;
        };
        let payloads = packetize_h264(&sealed);
        self.frame = self.frame.wrapping_add(1);
        for plain in self.sender.frame(payloads, frame.captured) {
            self.pacer.push_video(self.frame, plain);
        }
    }

    pub(crate) fn arrived(&mut self, readable: bool) {
        self.tally.datagrams += 1;
        if !readable {
            self.tally.unreadable += 1;
        }
    }

    pub(crate) fn on_feedback(
        &mut self,
        feedback: &Feedback,
        now: Instant,
        round_trip: Duration,
    ) -> Vec<BroadcastSignal> {
        self.tally.control += 1;
        let ssrc = self.sender.ssrc();
        match feedback {
            Feedback::Report(blocks) => {
                let mut signals = Vec::new();
                for block in blocks.iter().filter(|block| block.source == ssrc) {
                    let fraction = f64::from(block.fraction_lost) / LOSS_SCALE;
                    self.bandwidth.report(fraction, now, round_trip);
                    signals.extend(self.retarget());
                }
                signals
            }
            Feedback::Remb {
                bitrate,
                media: sources,
            } if sources.is_empty() || sources.contains(&ssrc) => {
                self.bandwidth.set_receiver_estimate(*bitrate);
                self.retarget().into_iter().collect()
            }
            Feedback::Nack {
                media: source,
                lost,
            } if *source == ssrc => {
                for &sequence in lost {
                    if let Some(repaired) = self.sender.repair(sequence) {
                        self.tally.repairs += 1;
                        self.pacer.push_urgent(repaired);
                    }
                }
                Vec::new()
            }
            Feedback::PictureLoss { media: source } if *source == ssrc => {
                self.tally.pictures += 1;
                let due = self.last_picture_request.is_none_or(|last_request| {
                    now.saturating_duration_since(last_request) >= PICTURE_EVERY
                });
                if !due {
                    return Vec::new();
                }
                self.last_picture_request = Some(now);
                vec![BroadcastSignal::KeyframeWanted]
            }
            _ => Vec::new(),
        }
    }

    pub(crate) fn wake_at(&self, now: Instant) -> Option<Instant> {
        self.pacer.wake_at(now)
    }

    pub(crate) fn release(&mut self, media: &mut Media, now: Instant) -> Vec<Vec<u8>> {
        let mut released = Vec::new();
        while let Some(outgoing) = self.pacer.pop(now) {
            if let Some(sealed) = media.seal_video(&outgoing.packet) {
                released.push(sealed);
            }
        }
        self.tally.packets += released.len() as u32;
        released
    }

    pub(crate) fn report(&self) -> Option<SenderInfo> {
        self.sender.report()
    }

    fn retarget(&mut self) -> Option<BroadcastSignal> {
        let target = self.bandwidth.rate_bps();
        self.pacer.set_rate(pacing(target));
        let moved = (target as f64 - self.reported as f64).abs() / self.reported.max(1) as f64;
        if moved < NOTABLE {
            return None;
        }
        self.reported = target;
        Some(BroadcastSignal::Bandwidth(target as u32))
    }
}

fn limits(ceiling: u64) -> Limits {
    let highest = ceiling.max(LOWEST_BPS);
    Limits {
        lowest_bps: LOWEST_BPS,
        start_bps: highest,
        highest_bps: highest,
    }
}

fn pacing(target: u64) -> u64 {
    (target as f64 * PACING) as u64
}
