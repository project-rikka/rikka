use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avutil::{self, AVRational};
use rikka_video_core::Result;
use rikka_video_core::decode::{Decoded, Decoder};
use rikka_video_core::demux::Demuxer;
use rikka_video_core::sw::SwConverter;

use crate::mailbox::{Delivery, Mailbox};

pub(crate) enum Control {
    Seek { target_us: i64, generation: u64 },
    SetLooping(bool),
    Stop,
}

pub(crate) enum Event {
    Frame(DecodedFrame),
    EndOfStream { generation: u64 },
    Looped,
    Fatal(String),
}

pub(crate) enum Payload {
    Hw(Frame),
    Sw {
        pixels: PixelBuffer,
        width: u32,
        height: u32,
    },
}

pub(crate) struct PixelBuffer {
    pixels: Vec<u8>,
    recycle: Sender<Vec<u8>>,
}

impl std::ops::Deref for PixelBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.pixels
    }
}

impl Drop for PixelBuffer {
    fn drop(&mut self) {
        let _ = self.recycle.send(std::mem::take(&mut self.pixels));
    }
}

pub(crate) struct DecodedFrame {
    pub pts_us: i64,
    pub generation: u64,
    pub payload: Payload,
}

pub(crate) struct Worker {
    demuxer: Demuxer,
    decoder: Decoder,
    converter: SwConverter,
    scratch: Frame,
    packet: Packet,
    mailbox: Arc<Mailbox>,
    recycle: Sender<Vec<u8>>,
    recycled: Receiver<Vec<u8>>,
    time_base: AVRational,
    generation: u64,
    looping: bool,
    draining: bool,
    synthetic_step_us: i64,
    last_pts_us: Option<i64>,
    sought_us: Option<i64>,
    pending_frame: Option<(i64, Frame)>,
}

enum Step {
    Continue,
    Stop,
}

impl Worker {
    pub(crate) fn new(demuxer: Demuxer, decoder: Decoder, mailbox: Arc<Mailbox>) -> Result<Self> {
        let (recycle, recycled) = channel();
        let time_base = demuxer.time_base();
        let frame_rate = demuxer.frame_rate();
        let synthetic_step_us = if frame_rate > 0.0 {
            (1_000_000.0 / frame_rate) as i64
        } else {
            33_333
        };
        Ok(Self {
            demuxer,
            decoder,
            converter: SwConverter::new(),
            scratch: Frame::new()?,
            packet: Packet::new()?,
            mailbox,
            recycle,
            recycled,
            time_base,
            generation: 0,
            looping: false,
            draining: false,
            synthetic_step_us,
            last_pts_us: None,
            sought_us: None,
            pending_frame: None,
        })
    }

    pub(crate) fn run(mut self) {
        loop {
            match self.step() {
                Ok(Step::Continue) => {}
                Ok(Step::Stop) => return,
                Err(err) => {
                    if let Err(error) = self.deliver(Event::Fatal(err.to_string())) {
                        tracing::debug!(%error, "failed to report a playback failure");
                    }
                    return;
                }
            }
        }
    }

    fn step(&mut self) -> Result<Step> {
        if let Step::Stop = self.poll_controls(false)? {
            return Ok(Step::Stop);
        }

        match self.decoder.receive(&mut self.scratch)? {
            Decoded::Frame => {
                let frame = std::mem::replace(&mut self.scratch, Frame::new()?);
                let pts_us = self.stamp(&frame);
                if self.precedes_seek_target(pts_us) {
                    self.pending_frame = Some((pts_us, frame));
                    return Ok(Step::Continue);
                }
                self.sought_us = None;
                self.pending_frame = None;
                let event = self.package(frame, pts_us)?;
                self.deliver(event)
            }
            Decoded::NeedsInput => {
                if self.draining {
                    return Ok(Step::Continue);
                }
                if self.demuxer.read(&mut self.packet)? {
                    let result = self.decoder.send(Some(&mut self.packet));
                    self.packet.unref();
                    result?;
                } else {
                    self.decoder.send(None)?;
                    self.draining = true;
                }
                Ok(Step::Continue)
            }
            Decoded::Eof => self.finish_stream(),
        }
    }

    fn finish_stream(&mut self) -> Result<Step> {
        if let Some((pts_us, frame)) = self.pending_frame.take() {
            self.sought_us = None;
            let event = self.package(frame, pts_us)?;
            if let Step::Stop = self.deliver(event)? {
                return Ok(Step::Stop);
            }
        }
        if self.looping {
            self.rewind(0)?;
            return self.deliver(Event::Looped);
        }
        let end = Event::EndOfStream {
            generation: self.generation,
        };
        match self.deliver(end)? {
            Step::Continue if self.draining => self.poll_controls(true),
            step => Ok(step),
        }
    }

    fn precedes_seek_target(&self, pts_us: i64) -> bool {
        self.sought_us
            .is_some_and(|sought| pts_us + self.synthetic_step_us <= sought)
    }

    fn rewind(&mut self, target_us: i64) -> Result<()> {
        self.demuxer.seek_us(target_us)?;
        self.decoder.flush();
        self.draining = false;
        self.last_pts_us = None;
        self.sought_us = (target_us > 0).then_some(target_us);
        self.pending_frame = None;
        Ok(())
    }

    fn stamp(&mut self, frame: &Frame) -> i64 {
        let pts_us = frame.pts_us(self.time_base).unwrap_or_else(|| {
            self.last_pts_us
                .map_or(0, |last| last + self.synthetic_step_us)
        });
        self.last_pts_us = Some(pts_us);
        pts_us
    }

    fn package(&mut self, frame: Frame, pts_us: i64) -> Result<Event> {
        let payload = if frame.format() == avutil::AV_PIX_FMT_VULKAN {
            Payload::Hw(frame)
        } else {
            let mut pixels = self.recycled.try_recv().unwrap_or_default();
            self.converter.rgba(&frame, &mut pixels)?;
            Payload::Sw {
                pixels: PixelBuffer {
                    pixels,
                    recycle: self.recycle.clone(),
                },
                width: frame.width() as u32,
                height: frame.height() as u32,
            }
        };
        Ok(Event::Frame(DecodedFrame {
            pts_us,
            generation: self.generation,
            payload,
        }))
    }

    fn deliver(&mut self, mut event: Event) -> Result<Step> {
        loop {
            match self.mailbox.deliver(event) {
                Delivery::Delivered => return Ok(Step::Continue),
                Delivery::Interrupted(returned, control) => {
                    if let Some(step) = self.apply_control(control)?.step() {
                        return Ok(step);
                    }
                    event = returned;
                }
            }
        }
    }

    fn poll_controls(&mut self, block: bool) -> Result<Step> {
        loop {
            let control = match block {
                true => self.mailbox.wait_control(),
                false => match self.mailbox.control() {
                    Some(control) => control,
                    None => return Ok(Step::Continue),
                },
            };
            if let Some(step) = self.apply_control(control)?.step() {
                return Ok(step);
            }
        }
    }

    fn apply_control(&mut self, control: Control) -> Result<Applied> {
        match control {
            Control::Stop => Ok(Applied::Stop),
            Control::SetLooping(looping) => {
                self.looping = looping;
                Ok(Applied::Handled)
            }
            Control::Seek {
                target_us,
                generation,
            } => {
                self.generation = generation;
                self.rewind(target_us)?;
                Ok(Applied::Resumed)
            }
        }
    }
}

enum Applied {
    Handled,
    Resumed,
    Stop,
}

impl Applied {
    fn step(self) -> Option<Step> {
        match self {
            Self::Handled => None,
            Self::Resumed => Some(Step::Continue),
            Self::Stop => Some(Step::Stop),
        }
    }
}
