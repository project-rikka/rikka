use rikka_remote_wire::{
    Chroma, Chromas, ClientMessage, Codec, CursorShape, DecodeError, Decoder, Depths, Display,
    EndReason, HEADER_BYTES, Header, Refusal, ServerMessage, SignIn, Stats, Stream, Viewport,
};

fn split(framed: &[u8]) -> (Header, &[u8]) {
    let header = Header::parse(framed[..HEADER_BYTES].try_into().unwrap()).unwrap();
    let body = &framed[HEADER_BYTES..];
    assert_eq!(header.body, body.len());
    (header, body)
}

#[test]
fn client_messages_round_trip() {
    let viewport = Viewport {
        width: 2560,
        height: 1440,
        refresh_mhz: 143_998,
    };
    let messages = [
        ClientMessage::Hello {
            version: 1,
            device: "desk".into(),
            platform: "linux".into(),
        },
        ClientMessage::SignIn(SignIn::Password {
            password: "パスワード".into(),
            remember: true,
        }),
        ClientMessage::SignIn(SignIn::Token(vec![7; 32])),
        ClientMessage::Start {
            viewport,
            decoders: vec![
                Decoder {
                    codec: Codec::Av1,
                    chromas: Chromas::YUV420,
                    depths: Depths::EIGHT.with(Depths::TEN),
                    max_width: 8192,
                    max_height: 8192,
                },
                Decoder {
                    codec: Codec::H265,
                    chromas: Chromas::YUV420.with(Chromas::YUV444),
                    depths: Depths::EIGHT,
                    max_width: 4096,
                    max_height: 2304,
                },
            ],
            sound: true,
        },
        ClientMessage::Viewport(viewport),
        ClientMessage::Keyframe,
        ClientMessage::PointerLock(true),
        ClientMessage::Clipboard {
            mime: "text/plain;charset=utf-8".into(),
            data: b"hello".to_vec(),
        },
        ClientMessage::Ping(u64::MAX),
        ClientMessage::SignOut,
    ];
    for message in messages {
        let framed = message.encode();
        let (header, body) = split(&framed);
        assert_eq!(ClientMessage::decode(header.kind, body), Ok(message));
    }
}

#[test]
fn server_messages_round_trip() {
    let messages = [
        ServerMessage::Welcome {
            version: 1,
            host: "nixos".into(),
            user: "eldo".into(),
        },
        ServerMessage::SignedIn { token: vec![1; 32] },
        ServerMessage::Refused {
            reason: Refusal::Throttled,
            retry_after_ms: 4000,
            detail: String::new(),
        },
        ServerMessage::Display(Display {
            name: "DP-1".into(),
            width: 3840,
            height: 2160,
            refresh_mhz: 60_000,
        }),
        ServerMessage::Stream(Stream {
            stream: 3,
            codec: Codec::H265,
            chroma: Chroma::Yuv420,
            depth: 8,
            width: 1920,
            height: 1080,
            frame_rate_mhz: 60_000,
            encoder: "Vulkan Video".into(),
        }),
        ServerMessage::CursorShape(CursorShape {
            serial: 9,
            width: 2,
            height: 1,
            hotspot_x: 1,
            hotspot_y: 0,
            rgba: vec![255, 0, 0, 255, 0, 0, 0, 0],
        }),
        ServerMessage::Cursor(0),
        ServerMessage::Clipboard {
            mime: "image/png".into(),
            data: vec![0x89, b'P', b'N', b'G'],
        },
        ServerMessage::Stats(Stats {
            target_bps: 20_000_000,
            estimate_bps: 24_000_000,
            frame_rate_mhz: 59_940,
            encode_us: 2100,
            protection_permille: 120,
            loss_permille: 4,
            rtt_us: 18_000,
        }),
        ServerMessage::Pong(5),
        ServerMessage::Ended(EndReason::Replaced),
        ServerMessage::FileSaved {
            id: 2,
            name: "a (1).txt".into(),
        },
        ServerMessage::FileFailed {
            id: 3,
            reason: "the disk is full".into(),
        },
    ];
    for message in messages {
        let framed = message.encode();
        let (header, body) = split(&framed);
        assert_eq!(ServerMessage::decode(header.kind, body), Ok(message));
    }
}

#[test]
fn headers_reject_empty_and_oversized_messages() {
    assert_eq!(Header::parse([0, 0, 0, 0, 1]), Err(DecodeError::Empty));
    let oversized = ((16_u32 << 20) + 1).to_le_bytes();
    assert!(matches!(
        Header::parse([oversized[0], oversized[1], oversized[2], oversized[3], 1]),
        Err(DecodeError::TooLong(_))
    ));
}

#[test]
fn unknown_message_types_are_reported_for_skipping() {
    assert_eq!(
        ServerMessage::decode(0xF0, &[]),
        Err(DecodeError::UnknownType(0xF0))
    );
}

#[test]
fn cursor_shapes_must_match_their_size() {
    let mut framed = ServerMessage::CursorShape(CursorShape {
        serial: 1,
        width: 1,
        height: 1,
        hotspot_x: 0,
        hotspot_y: 0,
        rgba: vec![0; 4],
    })
    .encode();
    framed.pop();
    let body = &framed[HEADER_BYTES..];
    assert!(ServerMessage::decode(0x86, body).is_err());
}
