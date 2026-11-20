mod cursor;
mod gpu;
mod sizing;

use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_capture::{BufferPool, Captured, Frames, Screen, ScreenOutput, Source};
use rikka_remote_link::VideoFrame;
use rikka_remote_wire::FrameFlags;
use rikka_vulkan_video::{Codec, Encoded, Encoder, Error as EncodeError, Rate, Request};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};

use crate::media::{
    Backlog, DisplayInfo, MediaError, MediaEvent, MediaSink, OutgoingFrame, StreamInfo, Viewport,
};

use cursor::{CursorNews, CursorTrack};
use gpu::Gpu;

pub(crate) use gpu::codecs;
pub(crate) use sizing::Size;

const THREAD: &str = "rikka-remote-video";
const SLOTS: usize = 3;
const REFINE_AFTER: Duration = Duration::from_millis(150);
const REFINE_QPS: [u8; 2] = [26, 20];
const PACING_SLACK: Duration = Duration::from_millis(1);
const MILLIHERTZ: f64 = 1000.0;
const REFERENCES: u8 = 16;
const BACKLOG_FRAMES: f64 = 1.5;
const LEAST_BACKLOG: Duration = Duration::from_millis(8);
const LOWEST_QP: u8 = 18;
const HIGHEST_QP: u8 = 40;
const VIRTUAL_BUFFER: Duration = Duration::from_millis(100);
const INTRA_BURST: Duration = Duration::from_millis(300);
const PREDICTED_FRAMES: f64 = 3.0;
const UNKNOWN_REFRESH_HZ: f64 = 60.0;

pub(crate) type VideoControl = Arc<CommandSender<VideoCommand>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Recover {
    Intra,
    From(u32),
}

pub(crate) enum VideoCommand {
    Bitrate(u64),
    Recover(Recover),
    Keyframe,
    Viewport(Viewport),
    PointerLock(bool),
    Display(Option<String>),
    Drained,
    Stop,
}

#[derive(Clone, Debug)]
pub(crate) struct VideoSetup {
    pub(crate) codec: Codec,
    pub(crate) viewport: Viewport,
    pub(crate) largest: Size,
    pub(crate) display: Option<String>,
    pub(crate) bitrate: u64,
}

pub(crate) struct VideoWorker {
    worker: Worker<VideoControl>,
}

impl VideoWorker {
    pub(crate) fn start(
        setup: VideoSetup,
        sink: MediaSink,
        backlog: Arc<Backlog>,
    ) -> Result<VideoWorker, MediaError> {
        let (commands, receiver) =
            command_channel(|| VideoCommand::Stop).map_err(|error| MediaError::Thread {
                thread: THREAD,
                error,
            })?;
        let worker = Worker::spawn(THREAD, Arc::new(commands), move || {
            let screen = Screen::connect().map_err(MediaError::Capture)?;
            let gpu = Gpu::lock()?;
            Ok(move || Pipeline::new(screen, gpu, receiver, sink, backlog, setup).run())
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => MediaError::Thread {
                thread: THREAD,
                error,
            },
            rikka_worker::Error::Startup(error) => error,
            rikka_worker::Error::Exited => MediaError::PictureGone,
        })?;
        Ok(VideoWorker { worker })
    }

    pub(crate) fn sender(&self) -> VideoControl {
        Arc::clone(self.worker.handle())
    }

    pub(crate) fn send(&self, command: VideoCommand) {
        if let Err(error) = self.worker.handle().send(command) {
            tracing::debug!("the picture thread is gone: {error}");
        }
    }
}

struct Watched {
    output: ScreenOutput,
    frames: Frames,
    cursor: Option<CursorTrack>,
}

struct Coder {
    encoder: Encoder,
    size: Size,
    frame_rate: u32,
    stream: u16,
    bitrate: u64,
}

#[derive(Clone, Copy, Debug)]
struct Sent {
    frame: u32,
    reference: u32,
    refinement: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Fresh,
    Refine(u8),
}

enum Flow {
    Continue,
    Stop,
}

struct Pipeline {
    screen: Screen,
    gpu: Gpu,
    commands: CommandReceiver<VideoCommand>,
    sink: MediaSink,
    backlog: Arc<Backlog>,
    codec: Codec,
    wanted: Option<String>,
    source: Option<Watched>,
    seen_changes: Option<u64>,
    slots: Option<BufferPool>,
    coder: Option<Coder>,
    bitrate: u64,
    viewport: Viewport,
    largest: Size,
    pointer_locked: bool,
    last_ready: Option<Instant>,
    last: Option<Sent>,
    refine: Option<(Instant, usize)>,
    keyframe_due: bool,
    recover: Option<Recover>,
    starved: bool,
    shown: Option<DisplayInfo>,
    next_stream: u16,
    start: Instant,
}

impl Pipeline {
    fn new(
        screen: Screen,
        gpu: Gpu,
        commands: CommandReceiver<VideoCommand>,
        sink: MediaSink,
        backlog: Arc<Backlog>,
        setup: VideoSetup,
    ) -> Pipeline {
        Pipeline {
            screen,
            gpu,
            commands,
            sink,
            backlog,
            codec: setup.codec,
            wanted: setup.display,
            source: None,
            seen_changes: None,
            slots: None,
            coder: None,
            bitrate: setup.bitrate,
            viewport: setup.viewport,
            largest: setup.largest,
            pointer_locked: false,
            last_ready: None,
            last: None,
            refine: None,
            keyframe_due: true,
            recover: None,
            starved: false,
            shown: None,
            next_stream: 0,
            start: Instant::now(),
        }
    }

    fn run(mut self) {
        loop {
            let commands: Vec<VideoCommand> = self.commands.drain().collect();
            for command in commands {
                if let Flow::Stop = self.command(command) {
                    return;
                }
            }
            if let Err(error) = self.turn() {
                self.emit(MediaEvent::Failed(error));
                return;
            }
            if let Err(error) = self.screen.flush() {
                self.emit(MediaEvent::Failed(MediaError::Compositor(error)));
                return;
            }
            let remaining = self
                .deadline()
                .map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let commands = &self.commands;
            if let Err(error) = self
                .screen
                .dispatch_with(|fd| commands.wait_readable(fd, remaining))
            {
                self.emit(MediaEvent::Failed(MediaError::Compositor(error)));
                return;
            }
            if let Err(error) = self.collect() {
                self.emit(MediaEvent::Failed(error));
                return;
            }
        }
    }

    fn emit(&self, event: MediaEvent) {
        if self.sink.send(event).is_err() {
            tracing::debug!("the viewer left before the picture stopped");
        }
    }

    fn command(&mut self, command: VideoCommand) -> Flow {
        match command {
            VideoCommand::Bitrate(bitrate) => self.bitrate = bitrate,
            VideoCommand::Recover(recover) => self.recover = Some(recover),
            VideoCommand::Keyframe => self.keyframe_due = true,
            VideoCommand::Viewport(viewport) => self.viewport = viewport,
            VideoCommand::PointerLock(locked) => self.pointer_locked = locked,
            VideoCommand::Display(display) => {
                self.wanted = display;
                self.seen_changes = None;
            }
            VideoCommand::Drained => self.starved = false,
            VideoCommand::Stop => return Flow::Stop,
        }
        Flow::Continue
    }

    fn turn(&mut self) -> Result<(), MediaError> {
        self.follow_display()?;
        let Some(source) = &mut self.source else {
            return Ok(());
        };
        if let Some(cursor) = &mut source.cursor {
            cursor.pump(&mut self.screen);
        }
        let constraints = source.frames.constraints();
        let pending = self.slots.as_ref().is_some_and(BufferPool::is_pending);
        if constraints.is_empty() || pending {
            return Ok(());
        }
        let stale = self
            .slots
            .as_ref()
            .is_none_or(|slots| slots.generation() != constraints.generation);
        if stale {
            self.coder = None;
            self.slots = None;
            self.slots = Some(
                BufferPool::dmabuf(&mut self.screen, self.gpu.device(), &constraints, SLOTS)
                    .map_err(MediaError::Buffer)?,
            );
            self.show(constraints.width, constraints.height);
        }
        self.prepare_coder(Size {
            width: constraints.width,
            height: constraints.height,
        })?;
        self.retune();
        let now = Instant::now();
        if let Some(latest) = self.slots.as_ref().and_then(BufferPool::latest) {
            if self.keyframe_due || self.recover.is_some() {
                self.encode(latest, now, Purpose::Fresh)?;
            } else if let Some((at, stage)) = self.refine
                && now >= at
                && self.backlog.queued().is_zero()
            {
                self.refine = REFINE_QPS
                    .get(stage + 1)
                    .map(|_| (now + REFINE_AFTER, stage + 1));
                self.encode(latest, now, Purpose::Refine(REFINE_QPS[stage]))?;
            }
        }
        if self.next_capture().is_none_or(|at| now >= at) {
            self.capture()?;
        }
        Ok(())
    }

    fn follow_display(&mut self) -> Result<(), MediaError> {
        let changes = self.screen.changes();
        let stopped = self
            .source
            .as_ref()
            .is_some_and(|source| source.frames.is_stopped());
        if self.seen_changes == Some(changes) && !stopped {
            return Ok(());
        }
        self.seen_changes = Some(changes);
        let outputs = self.screen.outputs();
        let chosen = match &self.wanted {
            Some(name) => outputs
                .iter()
                .find(|output| &output.name == name)
                .or_else(|| {
                    tracing::warn!(display = name, "no such display, showing the first one");
                    outputs.first()
                }),
            None => outputs.first(),
        };
        let Some(chosen) = chosen.cloned() else {
            self.source = None;
            return Ok(());
        };
        let current = self.source.as_ref().map(|source| &source.output);
        if !stopped && current.is_some_and(|output| output.name == chosen.name) {
            if let Some(source) = &mut self.source {
                source.output = chosen;
            }
            return Ok(());
        }
        let selected = Source::Output(chosen.name.clone());
        let frames = self
            .screen
            .frames(&selected, false)
            .map_err(|error| MediaError::Display {
                display: chosen.name.clone(),
                error,
            })?;
        let cursor = CursorTrack::open(&mut self.screen, &selected)
            .inspect_err(|error| tracing::warn!("{error}"))
            .ok();
        tracing::info!(display = chosen.name, "showing the display");
        self.source = Some(Watched {
            output: chosen,
            frames,
            cursor,
        });
        self.slots = None;
        self.coder = None;
        self.shown = None;
        self.keyframe_due = true;
        Ok(())
    }

    fn show(&mut self, width: u32, height: u32) {
        let Some(source) = &self.source else {
            return;
        };
        let output = &source.output;
        let display = DisplayInfo {
            name: output.name.clone(),
            width,
            height,
            refresh_hz: f64::from(output.refresh) / MILLIHERTZ,
        };
        if self.shown.as_ref() == Some(&display) {
            return;
        }
        self.shown = Some(display.clone());
        self.emit(MediaEvent::Display(display));
    }

    fn prepare_coder(&mut self, source: Size) -> Result<(), MediaError> {
        let refresh = self
            .shown
            .as_ref()
            .map_or(UNKNOWN_REFRESH_HZ, |display| display.refresh_hz);
        let size = sizing::encoded_size(source, self.viewport, self.largest);
        let frame_rate = sizing::frame_rate(refresh, self.viewport);
        if self
            .coder
            .as_ref()
            .is_some_and(|coder| coder.size == size && coder.frame_rate == frame_rate)
        {
            return Ok(());
        }
        self.coder = None;
        let encoder = Encoder::builder(self.gpu.device(), self.codec, size.width, size.height)
            .frame_rate(frame_rate)
            .references(REFERENCES)
            .rate(rate(self.bitrate, frame_rate))
            .build()?;
        if let Some(slots) = &mut self.slots {
            slots.bind(|spec| encoder.import(spec))?;
        }
        let stream = self.next_stream;
        self.next_stream = self.next_stream.wrapping_add(1);
        let (width, height) = encoder.size();
        self.emit(MediaEvent::Stream(StreamInfo {
            stream,
            codec: self.codec,
            width,
            height,
            frame_rate,
            encoder: encoder.name().to_owned(),
        }));
        self.coder = Some(Coder {
            encoder,
            size,
            frame_rate,
            stream,
            bitrate: self.bitrate,
        });
        self.last = None;
        self.keyframe_due = true;
        Ok(())
    }

    fn retune(&mut self) {
        let Some(coder) = &mut self.coder else {
            return;
        };
        if coder.bitrate != self.bitrate {
            coder.encoder.set_rate(rate(self.bitrate, coder.frame_rate));
            coder.bitrate = self.bitrate;
        }
    }

    fn interval(&self) -> Option<Duration> {
        let coder = self.coder.as_ref()?;
        Some(Duration::from_secs_f64(1.0 / f64::from(coder.frame_rate)))
    }

    fn next_capture(&self) -> Option<Instant> {
        let interval = self.interval()?;
        if self.starved {
            return None;
        }
        self.last_ready
            .map(|ready| ready + interval.saturating_sub(PACING_SLACK))
    }

    fn capture(&mut self) -> Result<(), MediaError> {
        let Some(interval) = self.interval() else {
            return Ok(());
        };
        let allowed = interval.mul_f64(BACKLOG_FRAMES).max(LEAST_BACKLOG);
        if self.backlog.queued() > allowed {
            self.starved = true;
            self.backlog.ask_drain(allowed);
            return Ok(());
        }
        let (Some(source), Some(slots)) = (&mut self.source, &mut self.slots) else {
            return Ok(());
        };
        slots
            .capture(&mut source.frames)
            .map_err(MediaError::Capture)?;
        Ok(())
    }

    fn deadline(&self) -> Option<Instant> {
        let pending = self.slots.as_ref().is_some_and(BufferPool::is_pending);
        let capture = (!pending).then(|| self.next_capture()).flatten();
        let refine = self.refine.map(|(at, _)| at);
        [capture, refine].into_iter().flatten().min()
    }

    fn collect(&mut self) -> Result<(), MediaError> {
        let Some(source) = &mut self.source else {
            return Ok(());
        };
        let mut news = Vec::new();
        if let Some(cursor) = &mut source.cursor {
            cursor.collect(self.pointer_locked, &mut news);
        }
        for item in news {
            self.emit(match item {
                CursorNews::Message(message) => MediaEvent::Message(message),
                CursorNews::Pointer { x, y } => MediaEvent::Pointer { x, y },
            });
        }
        let Some(source) = &mut self.source else {
            return Ok(());
        };
        for captured in source.frames.collect(self.slots.as_mut()) {
            match captured {
                Captured::Ready { index, .. } => {
                    let now = Instant::now();
                    self.last_ready = Some(now);
                    self.refine = Some((now + REFINE_AFTER, 0));
                    self.encode(index, now, Purpose::Fresh)?;
                }
                Captured::Lost => {}
                Captured::Ended => self.seen_changes = None,
            }
        }
        Ok(())
    }

    fn request(&mut self, purpose: Purpose) -> (Request, Option<u32>) {
        let Some(coder) = &self.coder else {
            return (Request::intra(), None);
        };
        let request = match (self.keyframe_due, self.recover.take(), self.last) {
            (true, _, _) | (_, _, None) => (Request::intra(), None),
            (false, Some(Recover::From(good)), _) if coder.encoder.holds(good) => {
                (Request::predicted(good), None)
            }
            (false, Some(_), _) => (Request::intra(), None),
            (false, None, Some(last))
                if last.refinement
                    && purpose == Purpose::Fresh
                    && self.backlog.holds(last.frame)
                    && coder.encoder.holds(last.reference) =>
            {
                (Request::predicted(last.reference), Some(last.frame))
            }
            (false, None, Some(last)) => (Request::predicted(last.frame), None),
        };
        match purpose {
            Purpose::Fresh => request,
            Purpose::Refine(qp) => (request.0.constant(qp), request.1),
        }
    }

    fn encode(
        &mut self,
        index: usize,
        captured: Instant,
        purpose: Purpose,
    ) -> Result<(), MediaError> {
        let (request, abandons) = self.request(purpose);
        let (Some(slots), Some(coder)) = (&self.slots, &mut self.coder) else {
            return Ok(());
        };
        let Some(source) = slots.imported(index) else {
            return Ok(());
        };
        let encoded: Encoded = match coder.encoder.encode(source, request) {
            Ok(encoded) => encoded,
            Err(EncodeError::Evicted(frame)) => {
                tracing::debug!(frame, "the reference left the ring; starting over");
                coder.encoder.encode(source, Request::intra())?
            }
            Err(error) => return Err(error.into()),
        };
        self.keyframe_due = false;
        let refinement = matches!(purpose, Purpose::Refine(_)) && !encoded.intra;
        self.last = Some(Sent {
            frame: encoded.frame,
            reference: encoded.reference,
            refinement,
        });
        let capture_us = captured.saturating_duration_since(self.start).as_micros() as u32;
        let frame = VideoFrame {
            stream: coder.stream,
            frame: encoded.frame,
            reference: encoded.reference,
            flags: FrameFlags {
                intra: encoded.intra,
                refinement,
            },
            capture_us,
            data: encoded.data,
        };
        self.emit(MediaEvent::Frame(OutgoingFrame {
            frame,
            abandons,
            encode: encoded.took,
            captured,
        }));
        Ok(())
    }
}

fn rate(bitrate: u64, frame_rate: u32) -> Rate {
    Rate {
        bitrate,
        buffer: VIRTUAL_BUFFER,
        min_qp: LOWEST_QP,
        max_qp: HIGHEST_QP,
        intra_burst: INTRA_BURST,
        predicted_burst: Duration::from_secs_f64(PREDICTED_FRAMES / f64::from(frame_rate.max(1))),
    }
}
