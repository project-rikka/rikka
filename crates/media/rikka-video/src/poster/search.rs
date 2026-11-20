use crate::error::{Error, Result};
use crate::poster::detail::Detail;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::demux::Demuxer;
use rikka_video_core::sw::{SwConverter, fitted};

const MOMENTS: [f64; 5] = [1.0 / 3.0, 2.0 / 3.0, 0.1, 0.9, 0.5];

const PACKETS_PER_FRAME: usize = 512;

const LOOKS_FROM_THE_START: usize = 5;

const SPACING_US: i64 = 1_000_000;

const FRAMES_BETWEEN_UNTIMED_LOOKS: u32 = 30;

const SAMPLE_SIDE: u32 = 64;

pub(super) struct Search<'a> {
    demuxer: &'a mut Demuxer,
    decoder: Decoder,
    packet: Packet,
    frame: Frame,
    drained: bool,
    moved: bool,
    sampler: SwConverter,
    sample: Vec<u8>,
    best: Option<Detail>,
    chosen: Frame,
}

impl<'a> Search<'a> {
    pub(super) fn new(demuxer: &'a mut Demuxer) -> Result<Self> {
        let decoder = Decoder::for_stills(
            demuxer.decoder(),
            demuxer.stream().codecpar,
            demuxer.time_base(),
        )?;
        demuxer.read_only_video();
        Ok(Self {
            demuxer,
            decoder,
            packet: Packet::new()?,
            frame: Frame::new()?,
            drained: false,
            moved: false,
            sampler: SwConverter::new(),
            sample: Vec::new(),
            best: None,
            chosen: Frame::new()?,
        })
    }

    pub(super) fn run(mut self) -> Result<Frame> {
        if !self.search_moments()? && self.best.is_none() {
            self.scan_opening()?;
        }
        match self.best {
            Some(_) => Ok(self.chosen),
            None => Err(Error::Unsupported("video has no frames".into())),
        }
    }

    fn search_moments(&mut self) -> Result<bool> {
        let Some(length) = self.demuxer.duration_us() else {
            return Ok(false);
        };
        let origin = self.demuxer.start_us().unwrap_or(0);
        for moment in MOMENTS {
            let target = origin.saturating_add((length as f64 * moment) as i64);
            self.moved = true;
            if let Err(error) = self.demuxer.seek_ahead_us(target) {
                tracing::debug!("no keyframe after {target} µs: {error}");
                continue;
            }
            self.restart();
            if self.next_frame() && self.score_frame()? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn scan_opening(&mut self) -> Result<()> {
        if self.moved {
            let origin = self.demuxer.start_us().unwrap_or(0);
            if let Err(error) = self.demuxer.seek_us(origin) {
                tracing::debug!("failed to seek to start: {error}");
            }
            self.restart();
        }
        let time_base = self.demuxer.time_base();
        let mut looks = 0;
        let mut due_at: Option<i64> = None;
        let mut untimed = 0;
        while looks < LOOKS_FROM_THE_START && self.next_frame() {
            let at = self.frame.pts_us(time_base);
            let due = match at {
                Some(at) => due_at.is_none_or(|due| at >= due),
                None => untimed % FRAMES_BETWEEN_UNTIMED_LOOKS == 0,
            };
            untimed += 1;
            if !due {
                continue;
            }
            looks += 1;
            due_at = at.map(|at| at.saturating_add(SPACING_US));
            if self.score_frame()? {
                break;
            }
        }
        Ok(())
    }

    fn restart(&mut self) {
        self.decoder.flush();
        self.drained = false;
    }

    fn next_frame(&mut self) -> bool {
        match self.decoded() {
            Ok(found) => found,
            Err(error) => {
                tracing::debug!("decoding stopped: {error}");
                false
            }
        }
    }

    fn decoded(&mut self) -> Result<bool> {
        let mut fed = 0;
        loop {
            match self.decoder.receive(&mut self.frame)? {
                Decoded::Frame if self.frame.is_corrupt() => self.frame.unref(),
                Decoded::Frame => return Ok(true),
                Decoded::Eof => return Ok(false),
                Decoded::NeedsInput if self.drained || fed == PACKETS_PER_FRAME => {
                    return Ok(false);
                }
                Decoded::NeedsInput => {
                    fed += 1;
                    if self.demuxer.read(&mut self.packet)? {
                        let sent = self.decoder.send(Some(&mut self.packet));
                        self.packet.unref();
                        if let Err(error) = sent {
                            tracing::debug!("skipping corrupt packet: {error}");
                        }
                    } else {
                        self.decoder.send(None)?;
                        self.drained = true;
                    }
                }
            }
        }
    }

    fn score_frame(&mut self) -> Result<bool> {
        let stored = (
            self.frame.width().max(1) as u32,
            self.frame.height().max(1) as u32,
        );
        let size = fitted(stored, Some((SAMPLE_SIDE, SAMPLE_SIDE)));
        self.sampler
            .rgba_sized(&self.frame, size, &mut self.sample)?;
        let detail = Detail::measure(&self.sample);
        if self.best.is_none_or(|best| detail > best) {
            self.chosen.hold(&self.frame)?;
            self.best = Some(detail);
        }
        Ok(detail.is_sufficient())
    }
}
