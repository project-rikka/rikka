use music_spotify_wire::{
    AccountType, Command, Credentials, Decoder, Encoder, Error, FileId, Handshake, Incoming, Keys,
    LoginError, Packet, Product, SpotifyId, StoredCredentials,
};

const HELLO: &[u8] = include_bytes!("fixtures/ap-hello.bin");
const RESPONSE: &[u8] = include_bytes!("fixtures/ap-response.bin");
const REPLY: &[u8] = include_bytes!("fixtures/ap-reply.bin");
const LOGIN: &[u8] = include_bytes!("fixtures/ap-login.bin");
const LOGIN_ANSWER: &[u8] = include_bytes!("fixtures/ap-login-answer.bin");
const DEVICE: &str = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";
const SERVER_KEY_BYTE: usize = 20;
const PAYLOAD_BYTE: usize = 4;
const CHUNK: usize = 5;
const PRODUCT: &str = concat!(
    r#"<?xml version="1.0" encoding="utf-8" ?>"#,
    "<products><product><type>premium</type><catalogue>premium</catalogue>",
    "<filter-explicit-content>1</filter-explicit-content>",
    "<head-files-url>https://heads-fa.spotify.com/head/{file_id}?a=1&b=2</head-files-url>",
    "</product></products>",
);

struct Recorded {
    bytes: Vec<u8>,
    at: usize,
}

impl rand::RngCore for Recorded {
    fn next_u32(&mut self) -> u32 {
        let mut word = [0u8; 4];
        self.fill_bytes(&mut word);
        u32::from_le_bytes(word)
    }

    fn next_u64(&mut self) -> u64 {
        let mut word = [0u8; 8];
        self.fill_bytes(&mut word);
        u64::from_le_bytes(word)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        for byte in destination {
            *byte = self.bytes[self.at % self.bytes.len()];
            self.at += 1;
        }
    }
}

impl rand::CryptoRng for Recorded {}

struct FakeAccessPoint {
    encoder: Encoder,
    decoder: Decoder,
}

impl FakeAccessPoint {
    fn answer(hello: &[u8]) -> &'static [u8] {
        assert_eq!(
            hello, HELLO,
            "the client hello differs from the one the AP accepted"
        );
        RESPONSE
    }

    fn accept(reply: &[u8], keys: &Keys) -> FakeAccessPoint {
        assert_eq!(
            reply, REPLY,
            "the client reply differs from the one the AP accepted"
        );
        FakeAccessPoint {
            encoder: Encoder::new(&keys.receive),
            decoder: Decoder::new(&keys.send),
        }
    }

    fn send(&mut self, command: u8, payload: &[u8]) -> Vec<u8> {
        self.encoder
            .encode(&Packet::new(Command::from_byte(command), payload.to_vec()))
            .unwrap()
    }

    fn read(&mut self, bytes: &[u8]) -> Packet {
        self.decoder.push(bytes);
        self.decoder.next_packet().unwrap().unwrap()
    }
}

fn recorded_randomness() -> Recorded {
    Recorded {
        bytes: (0u8..111)
            .map(|index| index.wrapping_mul(37).wrapping_add(11))
            .collect(),
        at: 0,
    }
}

fn credentials() -> Credentials {
    Credentials::Stored(StoredCredentials {
        username: "rikka-wire-test".to_owned(),
        auth_data: vec![0x5a; 64],
    })
}

fn connected() -> (Keys, FakeAccessPoint) {
    let handshake = Handshake::new(&mut recorded_randomness());
    let response = FakeAccessPoint::answer(handshake.hello());
    let established = handshake.finish(response).unwrap();
    let access_point = FakeAccessPoint::accept(&established.reply, &established.keys);
    (established.keys, access_point)
}

fn flipped(bytes: &[u8], index: usize) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes[index] ^= 0x01;
    bytes
}

#[test]
fn client_completes_exchange_recorded_from_spotify() {
    let (keys, _) = connected();
    let mut encoder = Encoder::new(&keys.send);
    let login = encoder
        .encode(&Packet::login(&credentials(), DEVICE))
        .unwrap();
    assert_eq!(login, LOGIN);

    let mut decoder = Decoder::new(&keys.receive);
    decoder.push(LOGIN_ANSWER);
    let answer = decoder.next_packet().unwrap().unwrap();
    assert_eq!(
        Incoming::read(&answer).unwrap(),
        Incoming::LoginFailed(LoginError::BadCredentials)
    );
}

#[test]
fn forged_server_key_is_refused() {
    let handshake = Handshake::new(&mut recorded_randomness());
    let forged = flipped(RESPONSE, SERVER_KEY_BYTE);
    assert!(matches!(handshake.finish(&forged), Err(Error::Signature)));
}

#[test]
fn tampered_packet_fails_authentication() {
    let (keys, _) = connected();
    let mut decoder = Decoder::new(&keys.receive);
    decoder.push(&flipped(LOGIN_ANSWER, PAYLOAD_BYTE));
    assert!(matches!(decoder.next_packet(), Err(Error::Mac)));
}

#[test]
fn session_packets_cross_the_fake_access_point() {
    let (keys, mut access_point) = connected();
    let mut decoder = Decoder::new(&keys.receive);
    let mut encoder = Encoder::new(&keys.send);

    let login = encoder
        .encode(&Packet::login(&credentials(), DEVICE))
        .unwrap();
    assert_eq!(access_point.read(&login).command, Command::Login);

    let welcome = [
        [0x52, 0x08].as_slice(),
        b"listener",
        &[0xf0, 0x01, 0x01, 0xc2, 0x02, 0x04, 0xa5, 0xa5, 0xa5, 0xa5],
    ]
    .concat();
    let key = [0x3c; 16];
    let mut stream = Vec::new();
    for (command, payload) in [
        (0xac, welcome),
        (0x04, 0x6553_f100u32.to_be_bytes().to_vec()),
        (0x1b, b"SE".to_vec()),
        (0x50, PRODUCT.as_bytes().to_vec()),
        (0x0d, [7u32.to_be_bytes().as_slice(), &key].concat()),
        (
            0x0e,
            [8u32.to_be_bytes().as_slice(), &[0x00, 0x01]].concat(),
        ),
        (0x76, vec![0x00, 0x01]),
    ] {
        stream.extend(access_point.send(command, &payload));
    }
    let mut received = Vec::new();
    for chunk in stream.chunks(CHUNK) {
        decoder.push(chunk);
        while let Some(packet) = decoder.next_packet().unwrap() {
            received.push(Incoming::read(&packet).unwrap());
        }
    }
    assert_eq!(
        received,
        [
            Incoming::Welcome(StoredCredentials {
                username: "listener".to_owned(),
                auth_data: vec![0xa5; 4],
            }),
            Incoming::Ping {
                server_time: 0x6553_f100
            },
            Incoming::CountryCode("SE".to_owned()),
            Incoming::ProductInfo(Product {
                account: Some(AccountType::Premium),
                catalogue: Some("premium".to_owned()),
                filters_explicit: true,
            }),
            Incoming::AudioKey { sequence: 7, key },
            Incoming::AudioKeyDenied { sequence: 8 },
            Incoming::Other(Command::Other(0x76)),
        ]
    );

    let pong = access_point.read(&encoder.encode(&Packet::pong()).unwrap());
    assert_eq!((pong.command, pong.payload), (Command::Pong, vec![0; 4]));

    let track = SpotifyId::from_base62("5sWHDYs0csV6RS48xBl0tH").unwrap();
    let file = FileId::new([0x22; FileId::LEN]);
    let request = access_point.read(
        &encoder
            .encode(&Packet::audio_key_request(7, track, file))
            .unwrap(),
    );
    assert_eq!(request.command, Command::RequestKey);
    assert_eq!(
        request.payload,
        [
            file.as_bytes().as_slice(),
            &track.to_gid(),
            &7u32.to_be_bytes(),
            &[0x00, 0x00],
        ]
        .concat()
    );
}

#[test]
fn product_info_names_other_accounts() {
    let free =
        "<products><product><type>free</type><catalogue>free</catalogue></product></products>";
    assert_eq!(
        Product::parse(free).unwrap(),
        Product {
            account: Some(AccountType::Other("free".to_owned())),
            catalogue: Some("free".to_owned()),
            filters_explicit: false,
        }
    );
}
