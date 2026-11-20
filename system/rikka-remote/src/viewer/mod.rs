mod codec;
mod control;
mod datagrams;
mod recovery;
mod sender;
mod streams;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rikka_remote_link::LinkError;
use rikka_remote_wire::{
    CLIPBOARD_TEXT, Chroma, ClientMessage, Decoder, Display, EndReason, LONGEST_CLIPBOARD,
    ServerMessage, SignIn, Stream, VERSION,
};
use rikka_session::remote::{Encoding, Viewer};
use rikka_vulkan_video::Codec;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::access::Admission;
use crate::media::{
    Backlog, InputCommand, InputWorker, Media, MediaError, MediaEvent, MediaSetup, Size,
    VideoCommand, VideoSetup, Viewport, codecs,
};
use crate::server::{Claim, Hub};
use crate::status::StatusChange;

use control::{ControlFlush, ControlWriter};
use sender::{Sender, Step};

const HANDSHAKE_PATIENCE: Duration = Duration::from_secs(15);
const SIGN_IN_PATIENCE: Duration = Duration::from_secs(600);
const MOST_SIGN_INS: u32 = 10;
const ENCODING_MOVE: f64 = 0.1;

#[derive(Debug, thiserror::Error)]
enum SessionError {
    #[error("the viewer said nothing in time")]
    Silent,
    #[error("{0}")]
    Link(#[from] LinkError),
    #[error("the viewer broke off the handshake")]
    Gone,
    #[error("the viewer sent {0} out of turn")]
    OutOfTurn(&'static str),
    #[error("the viewer tried too many passwords")]
    Guessing,
    #[error("{0}")]
    Media(#[from] MediaError),
    #[error("a media thread could not be joined: {0}")]
    Join(#[from] tokio::task::JoinError),
}

struct Greeting {
    device: String,
    token: Option<Vec<u8>>,
    viewport: rikka_remote_wire::Viewport,
    decoders: Vec<Decoder>,
    sound: bool,
}

pub(crate) async fn serve(hub: Arc<Hub>, incoming: quinn::Incoming) {
    let from = incoming.remote_address();
    let connection = match incoming.await {
        Ok(connection) => connection,
        Err(error) => {
            tracing::debug!(%from, "a viewer did not connect: {error}");
            return;
        }
    };
    match session(&hub, &connection).await {
        Ok(()) => {}
        Err(SessionError::Link(error)) if error.is_closed() => {
            tracing::debug!(%from, "a viewer session closed");
        }
        Err(error) => tracing::info!(%from, "a viewer session ended: {error}"),
    }
    connection.close(0_u32.into(), b"");
}

async fn session(hub: &Arc<Hub>, connection: &quinn::Connection) -> Result<(), SessionError> {
    let accepted = tokio::time::timeout(HANDSHAKE_PATIENCE, connection.accept_bi())
        .await
        .map_err(|_| SessionError::Silent)?;
    let (send, mut recv) = accepted.map_err(LinkError::from)?;
    let (writer, flush) = ControlWriter::spawn(send);
    let Some(greeting) = greet(hub, connection, &writer, &mut recv).await? else {
        part(writer, flush, None).await;
        return Ok(());
    };
    let encodable = match tokio::task::spawn_blocking(codecs).await? {
        Ok(encodable) => encodable,
        Err(error) => {
            hub.status
                .report(StatusChange::Problem(Some(error.to_string())));
            part(writer, flush, Some(EndReason::Stopped)).await;
            return Err(error.into());
        }
    };
    let Some(choice) = choose(&greeting.decoders, &encodable) else {
        part(writer, flush, Some(EndReason::Unsupported)).await;
        return Err(MediaError::NoCodec.into());
    };
    let claim = hub.claim().await;
    let id = claim.id;
    let opened = Opened {
        writer,
        flush,
        recv,
        greeting,
        choice,
        claim,
    };
    let result = live(hub, connection, opened).await;
    hub.release(id).await;
    hub.status.report(StatusChange::Viewer(None));
    result
}

async fn part(writer: ControlWriter, flush: ControlFlush, reason: Option<EndReason>) {
    if let Some(reason) = reason {
        writer.send(ServerMessage::Ended(reason));
    }
    drop(writer);
    flush.wait().await;
}

async fn next_within(
    recv: &mut quinn::RecvStream,
    patience: Duration,
) -> Result<ClientMessage, SessionError> {
    match tokio::time::timeout(patience, control::next(recv)).await {
        Ok(read) => read?.ok_or(SessionError::Gone),
        Err(_) => Err(SessionError::Silent),
    }
}

async fn greet(
    hub: &Hub,
    connection: &quinn::Connection,
    writer: &ControlWriter,
    recv: &mut quinn::RecvStream,
) -> Result<Option<Greeting>, SessionError> {
    let from = connection.remote_address().to_string();
    let (version, device) = match next_within(recv, HANDSHAKE_PATIENCE).await? {
        ClientMessage::Hello {
            version, device, ..
        } => (version, device),
        _ => return Err(SessionError::OutOfTurn("a message before Hello")),
    };
    if version != VERSION {
        writer.send(ServerMessage::Ended(EndReason::Unsupported));
        return Ok(None);
    }
    writer.send(ServerMessage::Welcome {
        version: VERSION,
        host: hub.identity.host.clone(),
        user: hub.identity.user.clone(),
    });
    let mut attempts = 0;
    let token = loop {
        attempts += 1;
        if attempts > MOST_SIGN_INS {
            return Err(SessionError::Guessing);
        }
        let request = match next_within(recv, SIGN_IN_PATIENCE).await? {
            ClientMessage::SignIn(request) => request,
            _ => return Err(SessionError::OutOfTurn("a message before signing in")),
        };
        let presented = match &request {
            SignIn::Token(token) => Some(token.clone()),
            SignIn::Password { .. } => None,
        };
        match hub.access.admit(request, &device, &from).await {
            Admission::Granted { token } => {
                let kept = presented.or_else(|| (!token.is_empty()).then(|| token.clone()));
                writer.send(ServerMessage::SignedIn { token });
                break kept;
            }
            Admission::Refused {
                reason,
                retry_after,
                detail,
            } => writer.send(ServerMessage::Refused {
                reason,
                retry_after_ms: retry_after.as_millis().min(u128::from(u32::MAX)) as u32,
                detail,
            }),
        }
    };
    match next_within(recv, HANDSHAKE_PATIENCE).await? {
        ClientMessage::Start {
            viewport,
            decoders,
            sound,
        } => Ok(Some(Greeting {
            device,
            token,
            viewport,
            decoders,
            sound,
        })),
        _ => Err(SessionError::OutOfTurn("a message before Start")),
    }
}

struct Choice {
    codec: Codec,
    largest: Size,
}

fn choose(decoders: &[Decoder], encodable: &[Codec]) -> Option<Choice> {
    decoders.iter().find_map(|decoder| {
        let codec = codec::from_wire(decoder.codec)?;
        (decoder.chromas.contains(Chroma::Yuv420)
            && decoder.depths.contains(8)
            && encodable.contains(&codec))
        .then_some(Choice {
            codec,
            largest: Size {
                width: u32::from(decoder.max_width),
                height: u32::from(decoder.max_height),
            },
        })
    })
}

struct Tasks(Vec<JoinHandle<()>>);

impl Drop for Tasks {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

struct Opened {
    writer: ControlWriter,
    flush: ControlFlush,
    recv: quinn::RecvStream,
    greeting: Greeting,
    choice: Choice,
    claim: Claim,
}

async fn start_media(
    setup: MediaSetup,
    sink: mpsc::UnboundedSender<MediaEvent>,
    backlog: Arc<Backlog>,
) -> Result<(InputWorker, Media), SessionError> {
    let input = tokio::task::spawn_blocking(InputWorker::start).await??;
    let media = tokio::task::spawn_blocking(move || Media::start(setup, sink, backlog)).await?;
    match media {
        Ok(media) => Ok((input, media)),
        Err(error) => {
            tokio::task::spawn_blocking(move || drop(input)).await?;
            Err(error.into())
        }
    }
}

async fn live(
    hub: &Arc<Hub>,
    connection: &quinn::Connection,
    opened: Opened,
) -> Result<(), SessionError> {
    let Opened {
        writer,
        flush,
        recv,
        greeting,
        choice,
        claim,
    } = opened;
    let Claim { mut end, _done, .. } = claim;
    let mut options = hub.options();
    let current = options.borrow_and_update().clone();
    let (sink, mut events) = mpsc::unbounded_channel::<MediaEvent>();
    let backlog = Arc::new(Backlog::default());
    let setup = MediaSetup {
        video: VideoSetup {
            codec: choice.codec,
            viewport: Viewport::from_wire(greeting.viewport),
            largest: choice.largest,
            display: current.display.clone(),
            bitrate: Sender::start_bitrate(current.max_bitrate),
        },
        sound: greeting.sound && current.sound,
    };
    let (input, mut media) = match start_media(setup, sink, Arc::clone(&backlog)).await {
        Ok(started) => started,
        Err(error) => {
            hub.status
                .report(StatusChange::Problem(Some(error.to_string())));
            part(writer, flush, Some(EndReason::Stopped)).await;
            return Err(error);
        }
    };
    let input_handle = input.handle();
    let mut sender = Sender::new(
        connection.clone(),
        current.max_bitrate,
        backlog,
        media.video(),
        writer.clone(),
    );
    let (feedback, mut reports) = mpsc::unbounded_channel();
    let (mut messages, reading) = control::forward(recv);
    let _tasks = Tasks(vec![
        reading,
        tokio::spawn(datagrams::read(
            connection.clone(),
            input_handle.clone(),
            feedback,
        )),
        tokio::spawn(streams::accept(
            connection.clone(),
            input_handle.clone(),
            writer.clone(),
            hub.notices.clone(),
        )),
    ]);
    let address = connection.remote_address().to_string();
    hub.notices.connected(&greeting.device);
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    let mut viewer = Viewer {
        address,
        device: greeting.device.clone(),
        since,
        encoding: None,
    };
    hub.status
        .report(StatusChange::Viewer(Some(viewer.clone())));
    let ended = loop {
        let now = Instant::now();
        let wake = tokio::time::Instant::from_std(sender.wake(now));
        tokio::select! {
            biased;
            reason = &mut end => break Some(reason.unwrap_or(EndReason::Stopped)),
            _ = connection.closed() => break None,
            event = events.recv() => match event {
                Some(MediaEvent::Frame(frame)) => sender.frame(frame, Instant::now()),
                Some(MediaEvent::Sound(packet)) => sender.sound(packet),
                Some(MediaEvent::Pointer { x, y }) => sender.pointer(x, y),
                Some(MediaEvent::Message(message)) => writer.send(message),
                Some(MediaEvent::Display(display)) => {
                    input_handle.send(InputCommand::Show(display.clone()));
                    writer.send(ServerMessage::Display(Display {
                        name: display.name,
                        width: display.width,
                        height: display.height,
                        refresh_mhz: (display.refresh_hz * 1000.0).round() as u32,
                    }));
                }
                Some(MediaEvent::Stream(stream)) => {
                    sender.begin_stream(stream.stream);
                    writer.send(ServerMessage::Stream(Stream {
                        stream: stream.stream,
                        codec: codec::to_wire(stream.codec),
                        chroma: Chroma::Yuv420,
                        depth: 8,
                        width: stream.width,
                        height: stream.height,
                        frame_rate_mhz: stream.frame_rate.saturating_mul(1000),
                        encoder: stream.encoder.clone(),
                    }));
                    viewer.encoding = Some(Encoding {
                        codec: codec::to_status(stream.codec),
                        width: stream.width,
                        height: stream.height,
                        bitrate: sender.video_bitrate(),
                        encoder: stream.encoder,
                    });
                    hub.status.report(StatusChange::Viewer(Some(viewer.clone())));
                }
                Some(MediaEvent::Failed(error)) => {
                    tracing::error!("the picture failed: {error}");
                    hub.status.report(StatusChange::Problem(Some(error.to_string())));
                    break Some(EndReason::Stopped);
                }
                None => break Some(EndReason::Stopped),
            },
            message = messages.recv() => match message {
                Some(message) => {
                    if let Some(reason) = control(message, &mut media, &writer) {
                        break Some(reason);
                    }
                }
                None => break None,
            },
            report = reports.recv() => {
                if let Some(report) = report {
                    sender.feedback(&report, Instant::now());
                }
            }
            changed = options.changed() => {
                if changed.is_ok() {
                    let next = options.borrow_and_update().clone();
                    media.set_sound(greeting.sound && next.sound);
                    media.tell(VideoCommand::Display(next.display.clone()));
                    sender.set_highest(next.max_bitrate);
                }
            }
            _ = tokio::time::sleep_until(wake) => {}
        }
        let now = Instant::now();
        sender.tick(now);
        if sender.pump(now) == Step::Lost {
            break None;
        }
        if let Some(encoding) = &mut viewer.encoding {
            let bitrate = sender.video_bitrate();
            let moved =
                (bitrate as f64 - encoding.bitrate as f64).abs() / encoding.bitrate.max(1) as f64;
            if moved >= ENCODING_MOVE {
                encoding.bitrate = bitrate;
                hub.status
                    .report(StatusChange::Viewer(Some(viewer.clone())));
            }
        }
    };
    if let Some(reason) = ended {
        writer.send(ServerMessage::Ended(reason));
    }
    if let (Some(EndReason::SignedOut), Some(token)) = (ended, &greeting.token) {
        hub.access.forget(token).await;
    }
    drop(writer);
    drop(sender);
    let stopping = (media, input);
    tokio::task::spawn_blocking(move || drop(stopping)).await?;
    flush.wait().await;
    Ok(())
}

fn control(message: ClientMessage, media: &mut Media, writer: &ControlWriter) -> Option<EndReason> {
    match message {
        ClientMessage::Viewport(viewport) => {
            media.tell(VideoCommand::Viewport(Viewport::from_wire(viewport)))
        }
        ClientMessage::Keyframe => media.tell(VideoCommand::Keyframe),
        ClientMessage::PointerLock(locked) => media.tell(VideoCommand::PointerLock(locked)),
        ClientMessage::Clipboard { mime, data } => {
            if mime == CLIPBOARD_TEXT && data.len() <= LONGEST_CLIPBOARD {
                match String::from_utf8(data) {
                    Ok(text) => media.set_clipboard(text),
                    Err(_) => tracing::debug!("the viewer's clipboard text is not UTF-8"),
                }
            }
        }
        ClientMessage::Ping(id) => writer.send(ServerMessage::Pong(id)),
        ClientMessage::SignOut => return Some(EndReason::SignedOut),
        ClientMessage::Hello { .. } | ClientMessage::SignIn(_) | ClientMessage::Start { .. } => {
            tracing::debug!("ignored a handshake message after the handshake");
        }
    }
    None
}
