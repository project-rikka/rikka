use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use rikka_remote_link::{
    Assembler, Chain, FeedbackBuilder, FrameReport, LinkError, SERVER_NAME, Trust, VideoFrame,
    client_config, client_endpoint, read_message,
};
use rikka_remote_wire::{
    CLIPBOARD_TEXT, Chromas, ClientDatagram, ClientMessage, Codec, DecodeError, Decoder, Depths,
    EndReason, FileHeader, InputEvent, Motion, Refusal, ServerDatagram, ServerMessage, SignIn,
    StreamKind, VERSION, Viewport,
};
use rikka_ui::Feed;
use tokio::io::AsyncReadExt;
use tokio::runtime::Runtime;
use tokio::sync::{Notify, mpsc};

use crate::session::clipboard::ClipboardSync;
use crate::session::sound::SoundWorker;
use crate::session::video::{Undecoded, VideoError, VideoWorker};
use crate::session::{Command, Refused, Report, Target};

const CONNECT_PATIENCE: Duration = Duration::from_secs(10);
const CLOSING_PATIENCE: Duration = Duration::from_millis(500);
const DEVICE_PATIENCE: Duration = Duration::from_secs(5);
const FEEDBACK_EVERY: Duration = Duration::from_millis(25);
const FEEDBACK_MARGIN: usize = 64;
const FALLBACK_DATAGRAM: usize = 1200;
const UPLOAD_CHUNK: usize = 256 * 1024;
const LARGEST_SIDE: u16 = 8192;
const WAITING_FRAMES: usize = 8;
const DEFAULT_VIEWPORT: Viewport = Viewport {
    width: 1920,
    height: 1080,
    refresh_mhz: 60_000,
};

#[derive(Debug, thiserror::Error)]
enum SessionError {
    #[error("{0}")]
    Link(#[from] LinkError),
    #[error("cannot find {0}")]
    Unresolved(String),
    #[error("cannot reach {0}: {1}")]
    Resolve(String, std::io::Error),
    #[error("the desktop did not answer in time")]
    Silent,
    #[error("the desktop answered out of turn")]
    OutOfTurn,
    #[error("the desktop closed the connection")]
    Closed,
    #[error(transparent)]
    Picture(#[from] VideoError),
}

type Reports = Arc<dyn Fn(Report) + Send + Sync>;

pub(super) fn run(
    runtime: Runtime,
    target: Target,
    feed: Feed,
    commands: mpsc::UnboundedReceiver<Command>,
    leave: Arc<Notify>,
    reports: impl Fn(Report) + Send + Sync + 'static,
) {
    let reports: Reports = Arc::new(reports);
    let result = runtime.block_on(async {
        let endpoint = client_endpoint()?;
        let result = tokio::select! {
            biased;
            result = session(&endpoint, target, feed, commands, Arc::clone(&reports)) => result,
            () = leave.notified() => Ok(()),
        };
        endpoint.close(0_u32.into(), b"");
        if tokio::time::timeout(CLOSING_PATIENCE, endpoint.wait_idle())
            .await
            .is_err()
        {
            tracing::debug!("the connection did not finish closing in time");
        }
        result
    });
    runtime.shutdown_background();
    match result {
        Ok(()) => {}
        Err(SessionError::Link(error)) if error.is_closed() => {
            reports(Report::Ended(EndReason::Stopped));
        }
        Err(error) => reports(Report::Failed(error.to_string())),
    }
}

async fn session(
    endpoint: &quinn::Endpoint,
    target: Target,
    feed: Feed,
    mut commands: mpsc::UnboundedReceiver<Command>,
    reports: Reports,
) -> Result<(), SessionError> {
    let address = resolve(&target.address).await?;
    let trust = match target.pin {
        Some(pin) => Trust::pinned(pin),
        None => Trust::first_use(),
    };
    let connecting = endpoint
        .connect_with(client_config(&trust)?, address, SERVER_NAME)
        .map_err(LinkError::from)?;
    let connection = match tokio::time::timeout(CONNECT_PATIENCE, connecting).await {
        Ok(Ok(connection)) => connection,
        Ok(Err(error)) => {
            if let (true, Some(seen)) = (trust.mismatched(), trust.seen()) {
                reports(Report::PinChanged { seen });
                return Ok(());
            }
            return Err(LinkError::from(error).into());
        }
        Err(_) => return Err(SessionError::Silent),
    };
    let (send, recv) = connection.open_bi().await.map_err(LinkError::from)?;
    let writer = Writer::spawn(send);
    let mut messages = forward(recv);
    writer.send(ClientMessage::Hello {
        version: VERSION,
        device: target.device.clone(),
        platform: std::env::consts::OS.to_owned(),
    });
    match next(&mut messages).await? {
        ServerMessage::Welcome { host, user, .. } => reports(Report::Connected {
            host,
            user,
            pin: trust.seen(),
        }),
        ServerMessage::Ended(reason) => {
            reports(Report::Ended(reason));
            return Ok(());
        }
        _ => return Err(SessionError::OutOfTurn),
    }
    let mut viewport = DEFAULT_VIEWPORT;
    let mut token = target.token.clone();
    let mut refused = None;
    loop {
        match token.take() {
            Some(presented) => writer.send(ClientMessage::SignIn(SignIn::Token(presented))),
            None => {
                reports(Report::Password(refused.take()));
                let Some((password, remember)) =
                    password(&mut commands, &mut viewport, &connection).await?
                else {
                    connection.close(0_u32.into(), b"");
                    return Ok(());
                };
                writer.send(ClientMessage::SignIn(SignIn::Password {
                    password: password.to_string(),
                    remember,
                }));
            }
        }
        match next(&mut messages).await? {
            ServerMessage::SignedIn { token } => {
                reports(Report::SignedIn {
                    token: (!token.is_empty()).then_some(token),
                });
                break;
            }
            ServerMessage::Refused {
                reason: Refusal::UnknownToken,
                ..
            } => reports(Report::TokenRefused),
            ServerMessage::Refused {
                reason,
                retry_after_ms,
                detail,
            } => {
                refused = Some(Refused {
                    reason,
                    retry_after_ms,
                    detail,
                });
            }
            ServerMessage::Ended(reason) => {
                reports(Report::Ended(reason));
                return Ok(());
            }
            _ => return Err(SessionError::OutOfTurn),
        }
    }
    let decoders = decoders(&feed).await;
    writer.send(ClientMessage::Start {
        viewport,
        decoders,
        sound: true,
    });
    Live::start(connection, writer, feed, reports)?
        .run(messages, commands)
        .await
}

async fn resolve(address: &str) -> Result<SocketAddr, SessionError> {
    let mut found = tokio::net::lookup_host(address)
        .await
        .map_err(|error| SessionError::Resolve(address.to_owned(), error))?;
    found
        .next()
        .ok_or_else(|| SessionError::Unresolved(address.to_owned()))
}

async fn password(
    commands: &mut mpsc::UnboundedReceiver<Command>,
    viewport: &mut Viewport,
    connection: &quinn::Connection,
) -> Result<Option<(zeroize::Zeroizing<String>, bool)>, SessionError> {
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::SignIn { password, remember }) => return Ok(Some((password, remember))),
                Some(Command::Viewport(shown)) => *viewport = shown,
                Some(Command::SignOut) | None => return Ok(None),
                Some(_) => {}
            },
            error = connection.closed() => return Err(LinkError::from(error).into()),
        }
    }
}

async fn decoders(feed: &Feed) -> Vec<Decoder> {
    let waiting = feed.clone();
    let device = tokio::task::spawn_blocking(move || waiting.wait_for_device(DEVICE_PATIENCE))
        .await
        .ok()
        .flatten();
    let gpu = crate::session::video::gpu_codecs(device.as_ref());
    let decoder = |codec| Decoder {
        codec,
        chromas: Chromas::YUV420,
        depths: Depths::EIGHT,
        max_width: LARGEST_SIDE,
        max_height: LARGEST_SIDE,
    };
    let mut listed: Vec<Decoder> = [Codec::H265, Codec::H264]
        .into_iter()
        .filter(|codec| gpu.contains(codec))
        .map(decoder)
        .collect();
    for codec in [Codec::H265, Codec::H264] {
        if !gpu.contains(&codec) {
            listed.push(decoder(codec));
        }
    }
    listed
}

struct Writer {
    bytes: mpsc::UnboundedSender<Vec<u8>>,
    task: tokio::task::JoinHandle<()>,
}

impl Writer {
    fn spawn(mut stream: quinn::SendStream) -> Writer {
        let (bytes, mut queued) = mpsc::unbounded_channel::<Vec<u8>>();
        let task = tokio::spawn(async move {
            while let Some(message) = queued.recv().await {
                if let Err(error) = stream.write_all(&message).await {
                    tracing::debug!("the control stream closed: {error}");
                    return;
                }
            }
            if let Err(error) = stream.finish() {
                tracing::debug!("cannot finish the control stream: {error}");
                return;
            }
            if let Err(error) = stream.stopped().await {
                tracing::debug!("the control stream ended unread: {error}");
            }
        });
        Writer { bytes, task }
    }

    fn send(&self, message: ClientMessage) {
        if self.bytes.send(message.encode()).is_err() {
            tracing::debug!("a control message after the stream closed");
        }
    }

    async fn flush(self) {
        drop(self.bytes);
        match tokio::time::timeout(CLOSING_PATIENCE, self.task).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::debug!("the control writer failed: {error}"),
            Err(_) => tracing::debug!("the desktop did not take the last messages in time"),
        }
    }
}

fn forward(
    mut recv: quinn::RecvStream,
) -> mpsc::UnboundedReceiver<Result<ServerMessage, LinkError>> {
    let (messages, received) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let read = match read_message(&mut recv).await {
                Ok(Some((kind, body))) => match ServerMessage::decode(kind, &body) {
                    Ok(message) => Ok(message),
                    Err(DecodeError::UnknownType(kind)) => {
                        tracing::debug!(kind, "skipped an unknown control message");
                        continue;
                    }
                    Err(error) => Err(LinkError::from(error)),
                },
                Ok(None) => return,
                Err(error) => Err(error),
            };
            let failed = read.is_err();
            if messages.send(read).is_err() || failed {
                return;
            }
        }
    });
    received
}

async fn next(
    messages: &mut mpsc::UnboundedReceiver<Result<ServerMessage, LinkError>>,
) -> Result<ServerMessage, SessionError> {
    match tokio::time::timeout(CONNECT_PATIENCE * 3, messages.recv()).await {
        Ok(Some(message)) => Ok(message?),
        Ok(None) => Err(SessionError::Closed),
        Err(_) => Err(SessionError::Silent),
    }
}

struct Live {
    connection: quinn::Connection,
    writer: Writer,
    reports: Reports,
    assembler: Assembler,
    chain: Chain,
    feedback: FeedbackBuilder,
    stream: Option<u16>,
    waiting: VecDeque<VideoFrame>,
    video: VideoWorker,
    sound: Option<SoundWorker>,
    clipboard: Option<ClipboardSync>,
    clipboard_changes: mpsc::UnboundedReceiver<String>,
    decode_failures: mpsc::UnboundedReceiver<Undecoded>,
    input: Option<mpsc::UnboundedSender<InputEvent>>,
    motion: u16,
    uploads: u32,
}

impl Live {
    fn start(
        connection: quinn::Connection,
        writer: Writer,
        feed: Feed,
        reports: Reports,
    ) -> Result<Live, SessionError> {
        let (failures, decode_failures) = mpsc::unbounded_channel();
        let video = VideoWorker::start(feed, failures)?;
        let sound = SoundWorker::start()
            .inspect_err(|error| tracing::warn!("no sound: {error}"))
            .ok();
        let (changes, clipboard_changes) = mpsc::unbounded_channel();
        let clipboard = ClipboardSync::start(changes)
            .inspect_err(|error| tracing::warn!("no clipboard: {error}"))
            .ok();
        Ok(Live {
            connection,
            writer,
            reports,
            assembler: Assembler::new(),
            chain: Chain::new(),
            feedback: FeedbackBuilder::new(Instant::now()),
            stream: None,
            waiting: VecDeque::new(),
            video,
            sound,
            clipboard,
            clipboard_changes,
            decode_failures,
            input: None,
            motion: 0,
            uploads: 0,
        })
    }

    async fn run(
        mut self,
        mut messages: mpsc::UnboundedReceiver<Result<ServerMessage, LinkError>>,
        mut commands: mpsc::UnboundedReceiver<Command>,
    ) -> Result<(), SessionError> {
        let mut ticks = tokio::time::interval(FEEDBACK_EVERY);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let expiry = self
                .assembler
                .next_deadline()
                .map(tokio::time::Instant::from_std);
            tokio::select! {
                biased;
                _ = async { tokio::time::sleep_until(expiry.unwrap_or_else(tokio::time::Instant::now)).await }, if expiry.is_some() => {
                    self.expire();
                }
                _ = ticks.tick() => self.send_feedback(),
                datagram = self.connection.read_datagram() => match datagram {
                    Ok(datagram) => self.datagram(&datagram),
                    Err(error) => {
                        self.closed(&mut messages, error).await;
                        return Ok(());
                    }
                },
                message = messages.recv() => match message {
                    Some(Ok(message)) => {
                        if self.message(message) {
                            return Ok(());
                        }
                    }
                    Some(Err(error)) => return Err(error.into()),
                    None => {
                        (self.reports)(Report::Ended(EndReason::Disconnected));
                        return Ok(());
                    }
                },
                command = commands.recv() => match command {
                    Some(command) => {
                        if self.command(command) {
                            let connection = self.connection.clone();
                            self.writer.flush().await;
                            connection.close(0_u32.into(), b"");
                            return Ok(());
                        }
                    }
                    None => {
                        self.connection.close(0_u32.into(), b"");
                        return Ok(());
                    }
                },
                undecoded = self.decode_failures.recv() => {
                    if let Some(undecoded) = undecoded {
                        self.chain.retract(undecoded.stream, undecoded.frame);
                        self.writer.send(ClientMessage::Keyframe);
                    }
                }
                changed = self.clipboard_changes.recv() => {
                    if let Some(text) = changed {
                        self.writer.send(ClientMessage::Clipboard {
                            mime: CLIPBOARD_TEXT.to_owned(),
                            data: text.into_bytes(),
                        });
                    }
                }
            }
        }
    }

    async fn closed(
        &mut self,
        messages: &mut mpsc::UnboundedReceiver<Result<ServerMessage, LinkError>>,
        error: quinn::ConnectionError,
    ) {
        while let Some(Ok(message)) = messages.recv().await {
            if self.message(message) {
                return;
            }
        }
        (self.reports)(Report::Failed(error.to_string()));
    }

    fn datagram(&mut self, bytes: &[u8]) {
        let now = Instant::now();
        let decoded = match ServerDatagram::decode(bytes) {
            Ok(decoded) => decoded,
            Err(error) => {
                tracing::debug!("a malformed datagram: {error}");
                return;
            }
        };
        self.feedback.received(decoded.seq(), now);
        match decoded {
            ServerDatagram::Video(shard) => {
                if let Some(frame) = self.assembler.push(&shard, now) {
                    self.arrived(frame);
                }
            }
            ServerDatagram::Audio(packet) => {
                if let Some(sound) = &self.sound {
                    sound.push(packet.timestamp, packet.primary, packet.previous);
                }
            }
            ServerDatagram::Pointer { x, y, .. } => (self.reports)(Report::Pointer { x, y }),
            ServerDatagram::Padding { .. } => {}
        }
    }

    fn arrived(&mut self, frame: VideoFrame) {
        if Some(frame.stream) == self.stream {
            self.decode(frame);
            return;
        }
        let announced_later = self
            .stream
            .is_none_or(|stream| (frame.stream.wrapping_sub(stream) as i16) > 0);
        if announced_later {
            if self.waiting.len() == WAITING_FRAMES {
                self.waiting.pop_front();
            }
            self.waiting.push_back(frame);
        }
    }

    fn decode(&mut self, frame: VideoFrame) {
        if !self.chain.decodable(&frame) {
            return;
        }
        if !self.video.has_room() {
            tracing::debug!(frame = frame.frame, "the decoder fell behind");
            self.writer.send(ClientMessage::Keyframe);
            return;
        }
        self.chain.decoded(&frame);
        self.video.push(frame);
    }

    fn expire(&mut self) {
        let lost = self.assembler.expire(Instant::now());
        if lost.is_empty() {
            return;
        }
        if let Some(stream) = self.assembler.stream() {
            for frame in lost {
                self.feedback.lost_frame(stream, frame);
            }
        }
        self.send_feedback();
    }

    fn send_feedback(&mut self) {
        let budget = self
            .connection
            .max_datagram_size()
            .unwrap_or(FALLBACK_DATAGRAM)
            .saturating_sub(FEEDBACK_MARGIN);
        let report = FrameReport {
            stream: self.stream.unwrap_or_default(),
            good: self.chain.newest_good(),
        };
        for feedback in self.feedback.build(report, budget) {
            let mut bytes = Vec::with_capacity(budget);
            ClientDatagram::Feedback(feedback).encode(&mut bytes);
            if let Err(error) = self.connection.send_datagram(Bytes::from(bytes)) {
                tracing::debug!("cannot send feedback: {error}");
            }
        }
    }

    fn message(&mut self, message: ServerMessage) -> bool {
        match message {
            ServerMessage::Display(display) => (self.reports)(Report::Display(display)),
            ServerMessage::Stream(stream) => {
                self.stream = Some(stream.stream);
                self.video.begin_stream(stream.clone());
                (self.reports)(Report::Stream(stream));
                let waiting = std::mem::take(&mut self.waiting);
                for frame in waiting {
                    if Some(frame.stream) == self.stream {
                        self.decode(frame);
                    }
                }
            }
            ServerMessage::CursorShape(shape) => (self.reports)(Report::CursorShape(shape)),
            ServerMessage::Cursor(serial) => (self.reports)(Report::Cursor(serial)),
            ServerMessage::PointerMode(mode) => (self.reports)(Report::PointerMode(mode)),
            ServerMessage::Clipboard { mime, data } => {
                if mime == CLIPBOARD_TEXT
                    && let (Ok(text), Some(clipboard)) = (String::from_utf8(data), &self.clipboard)
                {
                    clipboard.set(text);
                }
            }
            ServerMessage::Stats(stats) => (self.reports)(Report::Stats(stats)),
            ServerMessage::FileSaved { name, .. } => (self.reports)(Report::FileSaved(name)),
            ServerMessage::FileFailed { reason, .. } => (self.reports)(Report::FileFailed(reason)),
            ServerMessage::Ended(reason) => {
                (self.reports)(Report::Ended(reason));
                return true;
            }
            ServerMessage::Pong(_)
            | ServerMessage::Welcome { .. }
            | ServerMessage::SignedIn { .. }
            | ServerMessage::Refused { .. } => {}
        }
        false
    }

    fn command(&mut self, command: Command) -> bool {
        match command {
            Command::Viewport(viewport) => self.writer.send(ClientMessage::Viewport(viewport)),
            Command::Input(event) => self.input(event),
            Command::Motion(at) => {
                self.motion = self.motion.wrapping_add(1);
                let mut bytes = Vec::with_capacity(8);
                ClientDatagram::Motion(Motion {
                    seq: self.motion,
                    at,
                })
                .encode(&mut bytes);
                if let Err(error) = self.connection.send_datagram(Bytes::from(bytes)) {
                    tracing::debug!("cannot send motion: {error}");
                }
            }
            Command::PointerLock(locked) => self.writer.send(ClientMessage::PointerLock(locked)),
            Command::Keyframe => self.writer.send(ClientMessage::Keyframe),
            Command::Upload(path) => self.upload(path),
            Command::SignOut => {
                self.writer.send(ClientMessage::SignOut);
                return true;
            }
            Command::SignIn { .. } => {}
        }
        false
    }

    fn input(&mut self, event: InputEvent) {
        let input = self.input.get_or_insert_with(|| {
            let (events, mut queued) = mpsc::unbounded_channel::<InputEvent>();
            let connection = self.connection.clone();
            tokio::spawn(async move {
                let mut stream = match connection.open_uni().await {
                    Ok(stream) => stream,
                    Err(error) => {
                        tracing::warn!("cannot open the input stream: {error}");
                        return;
                    }
                };
                let mut bytes = vec![StreamKind::Input.to_wire()];
                while let Some(event) = queued.recv().await {
                    event.encode(&mut bytes);
                    while let Ok(more) = queued.try_recv() {
                        more.encode(&mut bytes);
                    }
                    if let Err(error) = stream.write_all(&bytes).await {
                        tracing::debug!("the input stream closed: {error}");
                        return;
                    }
                    bytes.clear();
                }
            });
            events
        });
        if input.send(event).is_err() {
            tracing::debug!("the input stream is gone");
        }
    }

    fn upload(&mut self, path: PathBuf) {
        self.uploads = self.uploads.wrapping_add(1);
        let id = self.uploads;
        let connection = self.connection.clone();
        let reports = Arc::clone(&self.reports);
        tokio::spawn(async move {
            if let Err(error) = send_file(&connection, id, &path).await {
                reports(Report::FileFailed(format!("{}: {error}", path.display())));
            }
        });
    }
}

async fn send_file(connection: &quinn::Connection, id: u32, path: &PathBuf) -> Result<(), String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| error.to_string())?;
    let size = file
        .metadata()
        .await
        .map_err(|error| error.to_string())?
        .len();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut stream = connection
        .open_uni()
        .await
        .map_err(|error| error.to_string())?;
    let mut header = vec![StreamKind::File.to_wire()];
    FileHeader { id, name, size }.encode(&mut header);
    stream
        .write_all(&header)
        .await
        .map_err(|error| error.to_string())?;
    let mut chunk = vec![0; UPLOAD_CHUNK];
    loop {
        let read = file
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        stream
            .write_all(&chunk[..read])
            .await
            .map_err(|error| error.to_string())?;
    }
    stream.finish().map_err(|error| error.to_string())
}
