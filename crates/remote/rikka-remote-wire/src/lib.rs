mod bytes;
mod control;
mod datagram;
mod error;
mod input;
mod types;

pub use control::{ClientMessage, HEADER_BYTES, Header, LONGEST_MESSAGE, ServerMessage, SignIn};
pub use datagram::{
    AUDIO_HEADER_BYTES, Arrival, AudioPacket, ClientDatagram, DELTA_UNIT_US, Feedback, FrameFlags,
    Motion, NO_FRAME, ServerDatagram, VIDEO_HEADER_BYTES, VideoShard, encode_padding,
    encode_pointer,
};
pub use error::DecodeError;
pub use input::{FileHeader, InputEvent, Parsed, Position};
pub use types::{
    Button, Chroma, Chromas, Codec, CursorShape, Decoder, Depths, Display, EndReason, PointerMode,
    Refusal, Stats, Stream, StreamKind, Viewport,
};

pub const ALPN: &[u8] = b"rikka-remote/1";
pub const VERSION: u16 = 2;
pub const DEFAULT_PORT: u16 = 47900;
pub const POSITION_EXTENT: u32 = 1 << 16;
pub const AUDIO_RATE: u32 = 48_000;
pub const AUDIO_FRAME_SAMPLES: u32 = 480;
pub const CURSOR_SHAPES: usize = 32;
pub const HIDDEN_CURSOR: u32 = 0;
pub const CLIPBOARD_TEXT: &str = "text/plain;charset=utf-8";
pub const LONGEST_CLIPBOARD: usize = 1 << 20;
