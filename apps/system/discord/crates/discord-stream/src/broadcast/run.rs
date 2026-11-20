use std::collections::HashMap;
use std::os::fd::AsFd;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use discord_client::BroadcastLine;
use discord_core::{EncodedVideo, VideoShape, VoiceEvent, VoiceEvents};
use futures_util::StreamExt as _;
use rikka_screencast::{BufferKey, Cast, Dmabuf, Frame, NextFrame, Picture, PixelOrder};
use rikka_video::{
    Coded, EncodeSpeed, Feed, Imported, PacketConfig, ParameterSets, PixelLayout, Previewer,
    VideoCodec, VideoPackets,
};
use rikka_worker::CommandReceiver;

use crate::broadcast::Command;
use crate::broadcast::preview::{self, Preview, SIDE, Shot};
use crate::picture::PictureSink;
use crate::quality::Quality;
use crate::update::{Outbox, StreamUpdate};

const APP: &str = "rikka-discord-stream";
const REPEAT_AFTER: Duration = Duration::from_millis(500);
const EARLY: f64 = 0.85;
const NO_PICTURE: Duration = Duration::from_secs(10);
const KEYFRAMES: Duration = Duration::from_secs(2);
const MOST_IMPORTED: usize = 16;

struct Gpu {
    previewer: Previewer,
    imported: HashMap<BufferKey, Imported>,
    device: rikka_gpu::Gpu,
}

struct Coder {
    video: VideoPackets,
    feed: Feed,
    source: (u32, u32),
}

struct Sending<'a> {
    cast: &'a Cast,
    line: &'a BroadcastLine,
    outbox: &'a Outbox,
    quality: Quality,
    coder: Option<Coder>,
    gpu: Option<Gpu>,
    shape: Option<VideoShape>,
    captured: Option<(u32, u32)>,
    started: Instant,
    origin: Option<Instant>,
    watched: bool,
    paused: bool,
    bandwidth: Option<u32>,
    last: Option<Frame>,
    sent_at: Option<Instant>,
    preview: Preview,
    thumbnail: &'a PictureSink,
    failure: Option<String>,
    keyframes: Duration,
    keyed_at: Option<Instant>,
}

pub(crate) fn run(
    mut events: VoiceEvents,
    cast: &Cast,
    line: &BroadcastLine,
    commands: &CommandReceiver<Command>,
    outbox: &Outbox,
    thumbnail: &PictureSink,
    quality: Quality,
) {
    let mut sending = Sending {
        cast,
        line,
        outbox,
        quality,
        coder: None,
        gpu: Gpu::open(cast),
        shape: None,
        captured: None,
        started: Instant::now(),
        origin: None,
        watched: true,
        paused: false,
        bandwidth: None,
        last: None,
        sent_at: None,
        preview: Preview::new(Instant::now()),
        thumbnail,
        failure: None,
        keyframes: KEYFRAMES,
        keyed_at: None,
    };
    let waker = commands.waker();
    let mut heard = Context::from_waker(&waker);
    loop {
        for command in commands.drain() {
            match command {
                Command::Pause(paused) => sending.pause(paused),
                Command::Quality(quality) => sending.apply_quality(quality),
                Command::Stop => {
                    line.show_video(None);
                    return;
                }
            }
        }
        if let Some(ended) = sending.drain_events(&mut events, &mut heard) {
            outbox.send(StreamUpdate::Ended(ended));
            return;
        }
        match cast.next_frame() {
            NextFrame::Frame(frame) => {
                if let Some(spent) = sending.offer(frame) {
                    cast.recycle(spent);
                }
            }
            NextFrame::Pending => {}
            NextFrame::Ended(why) => {
                line.show_video(None);
                outbox.send(StreamUpdate::Ended(why));
                return;
            }
        }
        sending.keep_alive();
        if let Some(failure) = sending.failure.take() {
            line.show_video(None);
            outbox.send(StreamUpdate::Ended(Some(failure)));
            return;
        }
        let timeout = sending
            .deadline()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        if let Err(error) = commands.wait_readable(cast.as_fd(), timeout) {
            line.show_video(None);
            outbox.send(StreamUpdate::Ended(Some(error.to_string())));
            return;
        }
    }
}

impl Gpu {
    fn open(cast: &Cast) -> Option<Gpu> {
        let device = rikka_gpu::Gpu::builder()
            .app_name(APP)
            .compute_support()
            .video_encode()
            .build()
            .inspect_err(|error| tracing::info!(%error, "streaming from memory: no GPU"))
            .ok()?;
        if !VideoPackets::takes_dmabufs(&device, VideoCodec::H264) {
            tracing::info!("streaming from memory: this GPU cannot encode H.264");
            return None;
        }
        let previewer = Previewer::new(&device)
            .inspect_err(|error| tracing::info!(%error, "streaming from memory"))
            .ok()?;
        cast.offer_dmabufs(device.dmabuf_texture_formats());
        Some(Gpu {
            previewer,
            imported: HashMap::new(),
            device,
        })
    }

    fn imported(&mut self, dmabuf: &Dmabuf, size: (u32, u32)) -> rikka_video::Result<&Imported> {
        import(&mut self.imported, &self.previewer, dmabuf, size)
    }

    fn shot(&mut self, dmabuf: &Dmabuf, size: (u32, u32)) -> rikka_video::Result<Shot> {
        let source = import(&mut self.imported, &self.previewer, dmabuf, size)?;
        let preview = self.previewer.preview(source, SIDE)?;
        Ok(Shot {
            width: preview.width,
            height: preview.height,
            rgba: preview.rgba,
        })
    }
}

fn import<'a>(
    imported: &'a mut HashMap<BufferKey, Imported>,
    previewer: &Previewer,
    dmabuf: &Dmabuf,
    size: (u32, u32),
) -> rikka_video::Result<&'a Imported> {
    if !imported.contains_key(&dmabuf.key()) {
        if imported.len() >= MOST_IMPORTED {
            imported.clear();
        }
        let made = dmabuf.with_spec(size, |spec| previewer.import(spec))?;
        imported.insert(dmabuf.key(), made);
    }
    Ok(&imported[&dmabuf.key()])
}

impl Sending<'_> {
    fn live(&self) -> bool {
        self.watched && !self.paused
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(1) / self.quality.frame_rate.per_second()
    }

    fn apply_quality(&mut self, quality: Quality) {
        if quality == self.quality && self.coder.is_some() {
            return;
        }
        self.quality = quality;
        self.coder = None;
    }

    fn pause(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        self.line.pause(paused);
        if paused {
            self.line.show_video(None);
            self.shape = None;
        } else if let Some(coder) = self.coder.as_mut() {
            coder.video.request_keyframe();
            self.shape = None;
        }
    }

    fn drain_events(
        &mut self,
        events: &mut VoiceEvents,
        heard: &mut Context<'_>,
    ) -> Option<Option<String>> {
        loop {
            match events.poll_next_unpin(heard) {
                Poll::Ready(Some(VoiceEvent::Ended(error))) => {
                    return Some(error.map(|error| error.to_string()));
                }
                Poll::Ready(Some(VoiceEvent::Viewers(viewers))) => {
                    self.outbox.send(StreamUpdate::Viewers(viewers));
                }
                Poll::Ready(Some(VoiceEvent::Wanted(quality))) => {
                    tracing::debug!(quality, "stream server requested quality");
                    let watched = quality > 0;
                    if watched != self.watched {
                        self.watched = watched;
                        self.outbox.send(StreamUpdate::Watched(watched));
                    }
                }
                Poll::Ready(Some(VoiceEvent::KeyframeWanted)) => {
                    tracing::debug!("viewer requested keyframe");
                    if let Some(coder) = self.coder.as_mut() {
                        coder.video.request_keyframe();
                    }
                    self.repeat_now();
                }
                Poll::Ready(Some(VoiceEvent::KeyframeEvery(every))) => self.keyframes = every,
                Poll::Ready(Some(VoiceEvent::Bandwidth(bitrate))) => {
                    self.bandwidth = Some(bitrate);
                    self.retune();
                }
                Poll::Ready(Some(_)) => {}
                Poll::Pending => return None,
                Poll::Ready(None) => {
                    return Some(Some("the stream connection closed".into()));
                }
            }
        }
    }

    fn offer(&mut self, frame: Frame) -> Option<Frame> {
        let now = Instant::now();
        let size = (frame.width, frame.height);
        if self.captured != Some(size) {
            self.captured = Some(size);
            if let Some(gpu) = &mut self.gpu {
                gpu.imported.clear();
            }
            self.outbox.send(StreamUpdate::Showing {
                width: size.0,
                height: size.1,
            });
        }
        if self.preview.thumbnail_due(now)
            && let Some(shot) = self.shot(&frame)
        {
            (self.thumbnail)(shot.width, shot.height, shot.rgba);
        }
        if !self.live() {
            return self.last.replace(frame);
        }
        let early = self.sent_at.is_some_and(|sent| {
            now.saturating_duration_since(sent) < self.interval().mul_f64(EARLY)
        });
        if early {
            return self.last.replace(frame);
        }
        self.send(&frame, now);
        self.last.replace(frame)
    }

    fn keep_alive(&mut self) {
        if self.captured.is_none() && self.started.elapsed() >= NO_PICTURE {
            self.failure = Some("the shared screen sent no picture".into());
            return;
        }
        let stale = self
            .sent_at
            .is_none_or(|sent| sent.elapsed() >= REPEAT_AFTER);
        if stale {
            self.repeat_now();
        }
    }

    fn deadline(&self) -> Option<Instant> {
        let picture = self.captured.is_none().then(|| self.started + NO_PICTURE);
        let repeat = (self.live() && self.last.is_some()).then(|| {
            self.sent_at
                .map_or_else(Instant::now, |sent| sent + REPEAT_AFTER)
        });
        picture.into_iter().chain(repeat).min()
    }

    fn repeat_now(&mut self) {
        if !self.live() {
            return;
        }
        if let Some(frame) = self.last.take() {
            self.send(&frame, Instant::now());
            self.last = Some(frame);
        }
    }

    fn shot(&mut self, frame: &Frame) -> Option<Shot> {
        let size = (frame.width, frame.height);
        match &frame.picture {
            Picture::Pixels(pixels) => Some(preview::shrink(pixels, size, SIDE)),
            Picture::Dmabuf(dmabuf) => match self.gpu.as_mut()?.shot(dmabuf, size) {
                Ok(shot) => Some(shot),
                Err(error) => {
                    self.leave_gpu(&error);
                    None
                }
            },
        }
    }

    fn send(&mut self, frame: &Frame, now: Instant) {
        self.sent_at = Some(now);
        let size = (frame.width, frame.height);
        let feed = match frame.picture {
            Picture::Pixels(_) => Feed::Memory,
            Picture::Dmabuf(_) => Feed::Dmabuf,
        };
        let stale = self
            .coder
            .as_ref()
            .is_none_or(|coder| coder.source != size || coder.feed != feed);
        if stale {
            self.open(size, feed);
        }
        let Some(coder) = self.coder.as_mut() else {
            return;
        };
        if self.shape.is_none() {
            let (width, height) = coder.video.size();
            let shape = VideoShape {
                width,
                height,
                frame_rate: self.quality.frame_rate.per_second(),
                bitrate: self.quality.bitrate((width, height)),
            };
            self.line.show_video(Some(shape));
            self.shape = Some(shape);
        }
        let origin = *self.origin.get_or_insert(now);
        let at = now.saturating_duration_since(origin);
        let due = self
            .keyed_at
            .is_none_or(|keyed| now.saturating_duration_since(keyed) >= self.keyframes);
        if due {
            coder.video.request_keyframe();
        }
        let coded = match (&frame.picture, &mut self.gpu) {
            (Picture::Pixels(pixels), _) => {
                let picture = rikka_video::Picture {
                    bytes: &pixels.bytes,
                    width: frame.width,
                    height: frame.height,
                    stride: pixels.stride,
                    layout: match pixels.order {
                        PixelOrder::Bgra => PixelLayout::Bgra,
                        PixelOrder::Rgba => PixelLayout::Rgba,
                    },
                };
                coder.video.encode(picture, at)
            }
            (Picture::Dmabuf(dmabuf), Some(gpu)) => gpu
                .imported(dmabuf, size)
                .and_then(|imported| coder.video.encode_imported(imported, at)),
            (Picture::Dmabuf(_), None) => return,
        };
        match coded {
            Ok(coded) => self.deliver(coded, now),
            Err(error) => {
                tracing::warn!(%error, "failed to encode shared frame");
                self.coder = None;
                if feed == Feed::Dmabuf {
                    self.leave_gpu(&error);
                }
            }
        }
        if self.preview.upload_due(now)
            && let Some(shot) = self.shot(frame)
        {
            match preview::jpeg(shot) {
                Ok(jpeg) => self.outbox.send(StreamUpdate::Preview(jpeg)),
                Err(error) => tracing::debug!(%error, "no stream preview available"),
            }
        }
    }

    fn deliver(&mut self, coded: Vec<Coded>, now: Instant) {
        for packet in coded {
            if packet.keyframe {
                self.keyed_at = Some(now);
            }
            self.line.send_video(EncodedVideo {
                data: packet.data.to_vec(),
                keyframe: packet.keyframe,
                captured: Duration::from_micros(packet.pts_us.max(0) as u64),
            });
        }
    }

    fn open(&mut self, source: (u32, u32), feed: Feed) {
        let size = self.quality.fit(source);
        let bitrate = self.quality.bitrate(size);
        let bitrate = self
            .bandwidth
            .map_or(bitrate, |estimate| estimate.min(bitrate));
        let config = PacketConfig::new(size.0, size.1)
            .frame_rate(self.quality.frame_rate.per_second())
            .codec(VideoCodec::H264)
            .bitrate(u64::from(bitrate))
            .speed(EncodeSpeed::Fast)
            .keyframes_on_request()
            .parameter_sets(ParameterSets::InBand)
            .feed(feed);
        self.shape = None;
        let device = self.gpu.as_ref().map(|gpu| &gpu.device);
        match VideoPackets::open(device, config) {
            Ok(video) => {
                self.coder = Some(Coder {
                    video,
                    feed,
                    source,
                });
            }
            Err(error) if feed == Feed::Dmabuf => {
                self.coder = None;
                self.leave_gpu(&error);
            }
            Err(error) => {
                self.coder = None;
                self.failure = Some(error.to_string());
            }
        }
    }

    fn leave_gpu(&mut self, error: &rikka_video::Error) {
        if self.gpu.take().is_none() {
            return;
        }
        tracing::warn!(%error, "the GPU cannot stream this screen; streaming from memory");
        self.cast.offer_dmabufs(&[]);
        self.last = None;
    }

    fn retune(&mut self) {
        let (Some(coder), Some(estimate)) = (self.coder.as_mut(), self.bandwidth) else {
            return;
        };
        let ceiling = self.quality.bitrate(coder.video.size());
        coder.video.set_bitrate(u64::from(estimate.min(ceiling)));
    }
}
