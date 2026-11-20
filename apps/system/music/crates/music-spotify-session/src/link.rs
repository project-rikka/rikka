use std::collections::HashMap;
use std::time::Duration;

use music_spotify_wire::{
    AUDIO_KEY_LEN, Credentials, Decoder, DeviceId, Encoder, FileId, Handshake, Incoming,
    LoginError, Packet, Product, SpotifyId, StoredCredentials,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep_until, timeout};

use crate::access_points::AccessPoint;
use crate::error::{Error, Result};

const OPEN_TIMEOUT: Duration = Duration::from_secs(10);
const KEY_ANSWER_TIMEOUT: Duration = Duration::from_millis(1500);
const INITIAL_PING_TIMEOUT: Duration = Duration::from_secs(20);
const PING_TIMEOUT: Duration = Duration::from_secs(80);
const PONG_DELAY: Duration = Duration::from_secs(60);
const PONG_ACK_TIMEOUT: Duration = Duration::from_secs(20);
const READ_CHUNK: usize = 16 * 1024;

pub(crate) type AudioKey = [u8; AUDIO_KEY_LEN];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Profile {
    pub(crate) country: Option<String>,
    pub(crate) product: Option<Product>,
}

pub(crate) struct Opened {
    stream: TcpStream,
    encoder: Encoder,
    decoder: Decoder,
    pub(crate) credentials: StoredCredentials,
}

pub(crate) struct Link {
    requests: mpsc::UnboundedSender<KeyRequest>,
    task: JoinHandle<()>,
}

struct KeyRequest {
    track: SpotifyId,
    file: FileId,
    answer: oneshot::Sender<Result<AudioKey>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KeepAlive {
    ExpectingPing,
    PendingPong,
    ExpectingPongAck,
}

struct Connection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    encoder: Encoder,
    decoder: Decoder,
    profile: watch::Sender<Profile>,
    pending: HashMap<u32, oneshot::Sender<Result<AudioKey>>>,
    sequence: u32,
    keepalive: KeepAlive,
    deadline: Instant,
}

pub(crate) async fn open(
    points: &[AccessPoint],
    credentials: &Credentials,
    device: &DeviceId,
) -> Result<Opened> {
    let mut failure = Error::TimedOut;
    for point in points {
        match timeout(OPEN_TIMEOUT, open_at(point, credentials, device)).await {
            Ok(Ok(opened)) => return Ok(opened),
            Ok(Err(Error::LoginFailed(code))) if code != LoginError::TryAnotherAccessPoint => {
                return Err(Error::LoginFailed(code));
            }
            Ok(Err(error)) => {
                tracing::debug!(
                    "Spotify access point {}:{} failed: {error}",
                    point.host,
                    point.port
                );
                failure = error;
            }
            Err(_) => {
                tracing::debug!(
                    "Spotify access point {}:{} timed out",
                    point.host,
                    point.port
                );
                failure = Error::TimedOut;
            }
        }
    }
    Err(failure)
}

async fn open_at(
    point: &AccessPoint,
    credentials: &Credentials,
    device: &DeviceId,
) -> Result<Opened> {
    let mut stream = TcpStream::connect((point.host.as_str(), point.port)).await?;
    stream.set_nodelay(true)?;
    let handshake = Handshake::new(&mut rand::rng());
    stream.write_all(handshake.hello()).await?;
    let mut header = [0u8; Handshake::RESPONSE_HEADER_LEN];
    stream.read_exact(&mut header).await?;
    let mut response = header.to_vec();
    response.resize(Handshake::response_len(header)?, 0);
    stream
        .read_exact(&mut response[Handshake::RESPONSE_HEADER_LEN..])
        .await?;
    let established = handshake.finish(&response)?;
    stream.write_all(&established.reply).await?;
    let mut encoder = Encoder::new(&established.keys.send);
    let mut decoder = Decoder::new(&established.keys.receive);
    let login = encoder.encode(&Packet::login(credentials, device.as_str()))?;
    stream.write_all(&login).await?;
    let mut chunk = vec![0u8; READ_CHUNK];
    let answer = loop {
        if let Some(packet) = decoder.next_packet()? {
            break packet;
        }
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Err(Error::Disconnected);
        }
        decoder.push(&chunk[..count]);
    };
    match Incoming::read(&answer)? {
        Incoming::Welcome(credentials) => Ok(Opened {
            stream,
            encoder,
            decoder,
            credentials,
        }),
        Incoming::LoginFailed(code) => Err(Error::LoginFailed(code)),
        _ => Err(Error::Protocol(music_spotify_wire::Error::Malformed(
            "login answer",
        ))),
    }
}

impl Link {
    pub(crate) fn start(opened: Opened, profile: watch::Sender<Profile>) -> Link {
        let (requests, receiving) = mpsc::unbounded_channel();
        let (reader, writer) = opened.stream.into_split();
        let connection = Connection {
            reader,
            writer,
            encoder: opened.encoder,
            decoder: opened.decoder,
            profile,
            pending: HashMap::new(),
            sequence: 0,
            keepalive: KeepAlive::ExpectingPing,
            deadline: Instant::now() + INITIAL_PING_TIMEOUT,
        };
        let task = tokio::spawn(async move {
            if let Err(error) = connection.run(receiving).await {
                tracing::info!("Spotify access point connection ended: {error}");
            }
        });
        Link { requests, task }
    }

    pub(crate) fn is_open(&self) -> bool {
        !self.requests.is_closed()
    }

    pub(crate) async fn closed(&self) {
        self.requests.closed().await;
    }

    pub(crate) async fn audio_key(&self, track: SpotifyId, file: FileId) -> Result<AudioKey> {
        let (answer, answered) = oneshot::channel();
        self.requests
            .send(KeyRequest {
                track,
                file,
                answer,
            })
            .map_err(|_| Error::Disconnected)?;
        match timeout(KEY_ANSWER_TIMEOUT, answered).await {
            Err(_) => Err(Error::TimedOut),
            Ok(Err(_)) => Err(Error::Disconnected),
            Ok(Ok(answer)) => answer,
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Connection {
    async fn run(mut self, mut requests: mpsc::UnboundedReceiver<KeyRequest>) -> Result<()> {
        let mut chunk = vec![0u8; READ_CHUNK];
        self.dispatch()?;
        loop {
            tokio::select! {
                read = self.reader.read(&mut chunk) => {
                    let count = read?;
                    if count == 0 {
                        return Err(Error::Disconnected);
                    }
                    self.decoder.push(&chunk[..count]);
                    self.dispatch()?;
                }
                request = requests.recv() => {
                    let Some(request) = request else {
                        return Ok(());
                    };
                    self.request_key(request).await?;
                }
                () = sleep_until(self.deadline) => self.keep_alive().await?,
            }
        }
    }

    fn dispatch(&mut self) -> Result<()> {
        while let Some(packet) = self.decoder.next_packet()? {
            match Incoming::read(&packet) {
                Ok(Incoming::Ping { .. }) => {
                    self.keepalive = KeepAlive::PendingPong;
                    self.deadline = Instant::now() + PONG_DELAY;
                }
                Ok(Incoming::PongAck) => {
                    self.keepalive = KeepAlive::ExpectingPing;
                    self.deadline = Instant::now() + PING_TIMEOUT;
                }
                Ok(Incoming::CountryCode(country)) => self
                    .profile
                    .send_modify(|profile| profile.country = Some(country)),
                Ok(Incoming::ProductInfo(product)) => self
                    .profile
                    .send_modify(|profile| profile.product = Some(product)),
                Ok(Incoming::AudioKey { sequence, key }) => self.answer(sequence, Ok(key)),
                Ok(Incoming::AudioKeyDenied { sequence }) => {
                    self.answer(sequence, Err(Error::KeyDenied));
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::debug!(
                        "ignoring unreadable Spotify {:?} packet: {error}",
                        packet.command
                    );
                }
            }
        }
        Ok(())
    }

    fn answer(&mut self, sequence: u32, key: Result<AudioKey>) {
        let Some(waiting) = self.pending.remove(&sequence) else {
            tracing::debug!("Spotify answered audio key {sequence} that nobody asked for");
            return;
        };
        if waiting.send(key).is_err() {
            tracing::debug!("audio key {sequence} arrived after its request gave up");
        }
    }

    async fn request_key(&mut self, request: KeyRequest) -> Result<()> {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        self.pending.insert(sequence, request.answer);
        self.send(&Packet::audio_key_request(
            sequence,
            request.track,
            request.file,
        ))
        .await
    }

    async fn keep_alive(&mut self) -> Result<()> {
        match self.keepalive {
            KeepAlive::PendingPong => {
                self.send(&Packet::pong()).await?;
                self.keepalive = KeepAlive::ExpectingPongAck;
                self.deadline = Instant::now() + PONG_ACK_TIMEOUT;
                Ok(())
            }
            KeepAlive::ExpectingPing | KeepAlive::ExpectingPongAck => Err(Error::KeepAliveLost),
        }
    }

    async fn send(&mut self, packet: &Packet) -> Result<()> {
        let frame = self.encoder.encode(packet)?;
        self.writer.write_all(&frame).await?;
        Ok(())
    }
}
