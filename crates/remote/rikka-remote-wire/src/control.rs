use crate::bytes::{Reader, Writer};
use crate::error::DecodeError;
use crate::types::{
    Chroma, Codec, CursorShape, Decoder, Display, EndReason, PointerMode, Refusal, Stats, Stream,
    Viewport,
};

pub const HEADER_BYTES: usize = 5;
pub const LONGEST_MESSAGE: u32 = 16 << 20;

const HELLO: u8 = 0x01;
const SIGN_IN: u8 = 0x02;
const START: u8 = 0x03;
const VIEWPORT: u8 = 0x04;
const KEYFRAME: u8 = 0x05;
const POINTER_LOCK: u8 = 0x06;
const CLIENT_CLIPBOARD: u8 = 0x07;
const PING: u8 = 0x08;
const SIGN_OUT: u8 = 0x09;

const WELCOME: u8 = 0x81;
const SIGNED_IN: u8 = 0x82;
const REFUSED: u8 = 0x83;
const DISPLAY: u8 = 0x84;
const STREAM: u8 = 0x85;
const CURSOR_SHAPE: u8 = 0x86;
const CURSOR: u8 = 0x87;
const SERVER_CLIPBOARD: u8 = 0x88;
const STATS: u8 = 0x89;
const PONG: u8 = 0x8A;
const ENDED: u8 = 0x8B;
const FILE_SAVED: u8 = 0x8C;
const FILE_FAILED: u8 = 0x8D;
const POINTER_MODE: u8 = 0x8E;

const PASSWORD: u8 = 0;
const TOKEN: u8 = 1;
const SOUND: u8 = 0b1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub kind: u8,
    pub body: usize,
}

impl Header {
    pub fn parse(bytes: [u8; HEADER_BYTES]) -> Result<Header, DecodeError> {
        let [a, b, c, d, kind] = bytes;
        let length = u32::from_le_bytes([a, b, c, d]);
        if length == 0 {
            return Err(DecodeError::Empty);
        }
        if length > LONGEST_MESSAGE {
            return Err(DecodeError::TooLong(u64::from(length)));
        }
        Ok(Header {
            kind,
            body: length as usize - 1,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum SignIn {
    Password { password: String, remember: bool },
    Token(Vec<u8>),
}

impl std::fmt::Debug for SignIn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignIn::Password { remember, .. } => formatter
                .debug_struct("Password")
                .field("remember", remember)
                .finish_non_exhaustive(),
            SignIn::Token(_) => formatter.write_str("Token"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientMessage {
    Hello {
        version: u16,
        device: String,
        platform: String,
    },
    SignIn(SignIn),
    Start {
        viewport: Viewport,
        decoders: Vec<Decoder>,
        sound: bool,
    },
    Viewport(Viewport),
    Keyframe,
    PointerLock(bool),
    Clipboard {
        mime: String,
        data: Vec<u8>,
    },
    Ping(u64),
    SignOut,
}

impl ClientMessage {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ClientMessage::Hello {
                version,
                device,
                platform,
            } => framed(HELLO, |writer| {
                writer.u16(*version).str(device).str(platform);
            }),
            ClientMessage::SignIn(SignIn::Password { password, remember }) => {
                framed(SIGN_IN, |writer| {
                    writer.u8(PASSWORD).str(password).bool(*remember);
                })
            }
            ClientMessage::SignIn(SignIn::Token(token)) => framed(SIGN_IN, |writer| {
                writer.u8(TOKEN).bytes(token);
            }),
            ClientMessage::Start {
                viewport,
                decoders,
                sound,
            } => framed(START, |writer| {
                viewport.write(writer);
                let listed = &decoders[..decoders.len().min(usize::from(u8::MAX))];
                writer.u8(listed.len() as u8);
                for decoder in listed {
                    decoder.write(writer);
                }
                writer.u8(if *sound { SOUND } else { 0 });
            }),
            ClientMessage::Viewport(viewport) => framed(VIEWPORT, |writer| viewport.write(writer)),
            ClientMessage::Keyframe => framed(KEYFRAME, |_| {}),
            ClientMessage::PointerLock(locked) => framed(POINTER_LOCK, |writer| {
                writer.bool(*locked);
            }),
            ClientMessage::Clipboard { mime, data } => framed(CLIENT_CLIPBOARD, |writer| {
                writer.str(mime).bytes(data);
            }),
            ClientMessage::Ping(id) => framed(PING, |writer| {
                writer.u64(*id);
            }),
            ClientMessage::SignOut => framed(SIGN_OUT, |_| {}),
        }
    }

    pub fn decode(kind: u8, body: &[u8]) -> Result<ClientMessage, DecodeError> {
        let mut reader = Reader::new(kind, body);
        let message = match kind {
            HELLO => ClientMessage::Hello {
                version: reader.u16()?,
                device: reader.str()?.to_owned(),
                platform: reader.str()?.to_owned(),
            },
            SIGN_IN => ClientMessage::SignIn(match reader.u8()? {
                PASSWORD => SignIn::Password {
                    password: reader.str()?.to_owned(),
                    remember: reader.bool()?,
                },
                TOKEN => SignIn::Token(reader.bytes()?.to_vec()),
                other => {
                    return Err(DecodeError::InvalidValue {
                        field: "sign-in method",
                        value: u64::from(other),
                    });
                }
            }),
            START => {
                let viewport = Viewport::read(&mut reader)?;
                let count = reader.u8()?;
                let decoders = (0..count)
                    .map(|_| Decoder::read(&mut reader))
                    .collect::<Result<Vec<_>, _>>()?;
                let flags = reader.u8()?;
                ClientMessage::Start {
                    viewport,
                    decoders,
                    sound: flags & SOUND != 0,
                }
            }
            VIEWPORT => ClientMessage::Viewport(Viewport::read(&mut reader)?),
            KEYFRAME => ClientMessage::Keyframe,
            POINTER_LOCK => ClientMessage::PointerLock(reader.bool()?),
            CLIENT_CLIPBOARD => ClientMessage::Clipboard {
                mime: reader.str()?.to_owned(),
                data: reader.bytes()?.to_vec(),
            },
            PING => ClientMessage::Ping(reader.u64()?),
            SIGN_OUT => ClientMessage::SignOut,
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerMessage {
    Welcome {
        version: u16,
        host: String,
        user: String,
    },
    SignedIn {
        token: Vec<u8>,
    },
    Refused {
        reason: Refusal,
        retry_after_ms: u32,
        detail: String,
    },
    Display(Display),
    Stream(Stream),
    CursorShape(CursorShape),
    Cursor(u32),
    Clipboard {
        mime: String,
        data: Vec<u8>,
    },
    Stats(Stats),
    Pong(u64),
    Ended(EndReason),
    FileSaved {
        id: u32,
        name: String,
    },
    FileFailed {
        id: u32,
        reason: String,
    },
    PointerMode(PointerMode),
}

impl ServerMessage {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ServerMessage::Welcome {
                version,
                host,
                user,
            } => framed(WELCOME, |writer| {
                writer.u16(*version).str(host).str(user);
            }),
            ServerMessage::SignedIn { token } => framed(SIGNED_IN, |writer| {
                writer.bytes(token);
            }),
            ServerMessage::Refused {
                reason,
                retry_after_ms,
                detail,
            } => framed(REFUSED, |writer| {
                writer.u8(reason.to_wire()).u32(*retry_after_ms).str(detail);
            }),
            ServerMessage::Display(display) => framed(DISPLAY, |writer| {
                writer
                    .str(&display.name)
                    .u32(display.width)
                    .u32(display.height)
                    .u32(display.refresh_mhz);
            }),
            ServerMessage::Stream(stream) => framed(STREAM, |writer| {
                writer
                    .u16(stream.stream)
                    .u8(stream.codec.to_wire())
                    .u8(stream.chroma.to_wire())
                    .u8(stream.depth)
                    .u32(stream.width)
                    .u32(stream.height)
                    .u32(stream.frame_rate_mhz)
                    .str(&stream.encoder);
            }),
            ServerMessage::CursorShape(shape) => framed(CURSOR_SHAPE, |writer| {
                writer
                    .u32(shape.serial)
                    .u16(shape.width)
                    .u16(shape.height)
                    .i16(shape.hotspot_x)
                    .i16(shape.hotspot_y)
                    .raw(&shape.rgba);
            }),
            ServerMessage::Cursor(serial) => framed(CURSOR, |writer| {
                writer.u32(*serial);
            }),
            ServerMessage::Clipboard { mime, data } => framed(SERVER_CLIPBOARD, |writer| {
                writer.str(mime).bytes(data);
            }),
            ServerMessage::Stats(stats) => framed(STATS, |writer| {
                writer
                    .u64(stats.target_bps)
                    .u64(stats.estimate_bps)
                    .u32(stats.frame_rate_mhz)
                    .u32(stats.encode_us)
                    .u16(stats.protection_permille)
                    .u16(stats.loss_permille)
                    .u32(stats.rtt_us);
            }),
            ServerMessage::Pong(id) => framed(PONG, |writer| {
                writer.u64(*id);
            }),
            ServerMessage::Ended(reason) => framed(ENDED, |writer| {
                writer.u8(reason.to_wire());
            }),
            ServerMessage::FileSaved { id, name } => framed(FILE_SAVED, |writer| {
                writer.u32(*id).str(name);
            }),
            ServerMessage::FileFailed { id, reason } => framed(FILE_FAILED, |writer| {
                writer.u32(*id).str(reason);
            }),
            ServerMessage::PointerMode(mode) => framed(POINTER_MODE, |writer| {
                writer.u8(mode.to_wire());
            }),
        }
    }

    pub fn decode(kind: u8, body: &[u8]) -> Result<ServerMessage, DecodeError> {
        let mut reader = Reader::new(kind, body);
        let message = match kind {
            WELCOME => ServerMessage::Welcome {
                version: reader.u16()?,
                host: reader.str()?.to_owned(),
                user: reader.str()?.to_owned(),
            },
            SIGNED_IN => ServerMessage::SignedIn {
                token: reader.bytes()?.to_vec(),
            },
            REFUSED => ServerMessage::Refused {
                reason: Refusal::from_wire(reader.u8()?)?,
                retry_after_ms: reader.u32()?,
                detail: reader.str()?.to_owned(),
            },
            DISPLAY => ServerMessage::Display(Display {
                name: reader.str()?.to_owned(),
                width: reader.u32()?,
                height: reader.u32()?,
                refresh_mhz: reader.u32()?,
            }),
            STREAM => ServerMessage::Stream(Stream {
                stream: reader.u16()?,
                codec: Codec::from_wire(reader.u8()?)?,
                chroma: Chroma::from_wire(reader.u8()?)?,
                depth: reader.u8()?,
                width: reader.u32()?,
                height: reader.u32()?,
                frame_rate_mhz: reader.u32()?,
                encoder: reader.str()?.to_owned(),
            }),
            CURSOR_SHAPE => {
                let serial = reader.u32()?;
                let width = reader.u16()?;
                let height = reader.u16()?;
                let hotspot_x = reader.i16()?;
                let hotspot_y = reader.i16()?;
                let rgba = reader.rest().to_vec();
                if rgba.len() != usize::from(width) * usize::from(height) * 4 {
                    return Err(DecodeError::InvalidValue {
                        field: "cursor bytes",
                        value: rgba.len() as u64,
                    });
                }
                ServerMessage::CursorShape(CursorShape {
                    serial,
                    width,
                    height,
                    hotspot_x,
                    hotspot_y,
                    rgba,
                })
            }
            CURSOR => ServerMessage::Cursor(reader.u32()?),
            SERVER_CLIPBOARD => ServerMessage::Clipboard {
                mime: reader.str()?.to_owned(),
                data: reader.bytes()?.to_vec(),
            },
            STATS => ServerMessage::Stats(Stats {
                target_bps: reader.u64()?,
                estimate_bps: reader.u64()?,
                frame_rate_mhz: reader.u32()?,
                encode_us: reader.u32()?,
                protection_permille: reader.u16()?,
                loss_permille: reader.u16()?,
                rtt_us: reader.u32()?,
            }),
            PONG => ServerMessage::Pong(reader.u64()?),
            ENDED => ServerMessage::Ended(EndReason::from_wire(reader.u8()?)?),
            FILE_SAVED => ServerMessage::FileSaved {
                id: reader.u32()?,
                name: reader.str()?.to_owned(),
            },
            FILE_FAILED => ServerMessage::FileFailed {
                id: reader.u32()?,
                reason: reader.str()?.to_owned(),
            },
            POINTER_MODE => ServerMessage::PointerMode(PointerMode::from_wire(reader.u8()?)?),
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(message)
    }
}

fn framed(kind: u8, body: impl FnOnce(&mut Writer<'_>)) -> Vec<u8> {
    let mut bytes = vec![0; 4];
    bytes.push(kind);
    body(&mut Writer(&mut bytes));
    let length = u32::try_from(bytes.len() - 4).unwrap_or(u32::MAX);
    bytes[..4].copy_from_slice(&length.to_le_bytes());
    bytes
}
