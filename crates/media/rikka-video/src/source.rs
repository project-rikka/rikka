use std::path::Path;
use std::time::Duration;

use crate::error::Result;
use crate::read::VideoInfo;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avformat::AVStream;
use rikka_ffmpeg::avutil::{self, AVRational};
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::demux::Demuxer;

const NANOS: AVRational = AVRational {
    num: 1,
    den: 1_000_000_000,
};

pub(crate) struct Source {
    demuxer: Demuxer,
    decoder: Decoder,
    packet: Packet,
    time_base: AVRational,
    origin: i64,
    step: Duration,
    draining: bool,
    last: Option<Duration>,
}

impl Source {
    pub(crate) fn open(path: &Path) -> Result<(Self, VideoInfo)> {
        let demuxer = Demuxer::open(path)?;
        let decoder = Decoder::open(
            demuxer.decoder(),
            demuxer.stream().codecpar,
            demuxer.time_base(),
            None,
        )?;
        Self::from_parts(demuxer, decoder)
    }

    pub(crate) fn for_stills(path: &Path) -> Result<(Self, VideoInfo)> {
        let mut demuxer = Demuxer::open(path)?;
        demuxer.read_only_video();
        let decoder = Decoder::for_stills(
            demuxer.decoder(),
            demuxer.stream().codecpar,
            demuxer.time_base(),
        )?;
        Self::from_parts(demuxer, decoder)
    }

    fn from_parts(demuxer: Demuxer, decoder: Decoder) -> Result<(Self, VideoInfo)> {
        let info = VideoInfo::from_demuxer(&demuxer)?;
        let time_base = demuxer.time_base();
        let origin = demuxer.start_us().unwrap_or(0).saturating_mul(1_000);
        let step = match info.frame_rate > 0.0 {
            true => Duration::from_secs_f64(1.0 / info.frame_rate),
            false => Duration::from_millis(40),
        };
        let source = Self {
            demuxer,
            decoder,
            packet: Packet::new()?,
            time_base,
            origin,
            step,
            draining: false,
            last: None,
        };
        Ok((source, info))
    }

    pub(crate) fn stream(&self) -> &AVStream {
        self.demuxer.stream()
    }

    pub(crate) fn seek(&mut self, to: Duration) -> Result<bool> {
        let target = self.origin.saturating_add(nanos(to)) / 1_000;
        let landed = match self.demuxer.seek_us(target) {
            Ok(()) => true,
            Err(error) if !to.is_zero() => {
                tracing::debug!("failed to seek to {to:?}, reading from start: {error}");
                self.demuxer.seek_us(self.origin / 1_000)?;
                false
            }
            Err(error) => return Err(error.into()),
        };
        self.restart();
        Ok(landed)
    }

    pub(crate) fn seek_ahead(&mut self, to: Duration) -> bool {
        let target = self.origin.saturating_add(nanos(to)) / 1_000;
        let landed = self.demuxer.seek_ahead_us(target).is_ok();
        if landed {
            self.restart();
        }
        landed
    }

    fn restart(&mut self) {
        self.decoder.flush();
        self.draining = false;
        self.last = None;
    }

    pub(crate) fn next(&mut self, frame: &mut Frame) -> Result<Option<Duration>> {
        loop {
            match self.decoder.receive(frame) {
                Ok(Decoded::Frame) => return Ok(Some(self.stamp(frame))),
                Ok(Decoded::Eof) => return Ok(None),
                Ok(Decoded::NeedsInput) if self.draining => return Ok(None),
                Ok(Decoded::NeedsInput) => self.feed()?,
                Err(error) if damaged(&error) => {
                    tracing::debug!("skipping corrupt frame: {error}");
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(crate) fn length(&self, frame: &Frame) -> Duration {
        match frame.span() > 0 {
            true => Duration::from_nanos(self.rescale_to_nanos(frame.span()).max(1) as u64),
            false => self.step,
        }
    }

    pub(crate) fn keyframe_before(&self, at: Duration) -> Option<Duration> {
        self.demuxer
            .keyframe(self.timestamp(at), true)
            .map(|timestamp| self.time(timestamp))
    }

    pub(crate) fn keyframe_after(&self, at: Duration) -> Option<Duration> {
        self.demuxer
            .keyframe(self.timestamp(at), false)
            .map(|timestamp| self.time(timestamp))
    }

    pub(crate) fn indexed_beyond(&self, at: Duration) -> bool {
        self.demuxer.keyframe(self.timestamp(at), false).is_some()
    }

    fn feed(&mut self) -> Result<()> {
        let read = self.demuxer.read(&mut self.packet).unwrap_or_else(|error| {
            tracing::warn!("demuxing stopped early: {error}");
            false
        });
        if !read {
            self.decoder.send(None)?;
            self.draining = true;
            return Ok(());
        }
        let sent = self.decoder.send(Some(&mut self.packet));
        self.packet.unref();
        match sent {
            Err(error) if damaged(&error) => {
                tracing::debug!("skipping corrupt packet: {error}");
                Ok(())
            }
            sent => Ok(sent?),
        }
    }

    fn stamp(&mut self, frame: &Frame) -> Duration {
        let stamped = frame.timestamp().map(|timestamp| self.time(timestamp));
        let pts = match (stamped, self.last) {
            (Some(pts), Some(last)) if pts <= last => last + Duration::from_nanos(1),
            (Some(pts), _) => pts,
            (None, Some(last)) => last + self.step,
            (None, None) => Duration::ZERO,
        };
        self.last = Some(pts);
        pts
    }

    fn time(&self, timestamp: i64) -> Duration {
        let nanos = self.rescale_to_nanos(timestamp).saturating_sub(self.origin);
        Duration::from_nanos(nanos.max(0) as u64)
    }

    fn rescale_to_nanos(&self, timestamp: i64) -> i64 {
        unsafe { avutil::av_rescale_q(timestamp, self.time_base, NANOS) }
    }

    fn timestamp(&self, at: Duration) -> i64 {
        unsafe {
            avutil::av_rescale_q(self.origin.saturating_add(nanos(at)), NANOS, self.time_base)
        }
    }
}

fn nanos(at: Duration) -> i64 {
    i64::try_from(at.as_nanos()).unwrap_or(i64::MAX)
}

fn damaged(error: &rikka_video_core::Error) -> bool {
    matches!(
        error,
        rikka_video_core::Error::Av {
            code: avutil::AVERROR_INVALIDDATA,
            ..
        }
    )
}
