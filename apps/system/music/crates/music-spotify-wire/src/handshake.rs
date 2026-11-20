use hmac::{Hmac, KeyInit, Mac as _};
use prost::Message as _;
use rand::CryptoRng;
use sha1::Sha1;

use crate::diffie_hellman::DhSecret;
use crate::error::{Error, Result};
use crate::login::LoginError;
use crate::messages::{
    ApResponseMessage, BuildInfo, ClientHello, ClientResponsePlaintext, CryptoResponseUnion,
    LoginCryptoDiffieHellmanHello, LoginCryptoDiffieHellmanResponse, LoginCryptoHelloUnion,
    LoginCryptoResponseUnion, PowResponseUnion,
};

const HELLO_MAGIC: [u8; 2] = [0x00, 0x04];
const LENGTH_BYTES: usize = 4;
const RESPONSE_LIMIT: usize = 64 * 1024;
const NONCE_BYTES: usize = 16;
const PADDING: u8 = 0x1e;
const PRODUCT_CLIENT: i32 = 0;
const PRODUCT_FLAG_NONE: i32 = 0;
const CRYPTO_SUITE_SHANNON: i32 = 0;
const SERVER_KEYS_KNOWN: u32 = 1;
const SPOTIFY_VERSION: u64 = 124_200_290;
const DERIVED_ROUNDS: u8 = 5;
const CHALLENGE_KEY: std::ops::Range<usize> = 0..20;
const SEND_KEY: std::ops::Range<usize> = 20..52;
const RECEIVE_KEY: std::ops::Range<usize> = 52..84;

pub const KEY_LEN: usize = 32;

#[derive(Debug)]
pub struct Handshake {
    secret: DhSecret,
    hello: Vec<u8>,
}

#[derive(Debug)]
pub struct Established {
    pub reply: Vec<u8>,
    pub keys: Keys,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Keys {
    pub send: [u8; KEY_LEN],
    pub receive: [u8; KEY_LEN],
}

impl Handshake {
    pub const RESPONSE_HEADER_LEN: usize = LENGTH_BYTES;

    pub fn new<R: CryptoRng + ?Sized>(rng: &mut R) -> Handshake {
        let secret = DhSecret::generate(rng);
        let mut nonce = vec![0u8; NONCE_BYTES];
        rng.fill_bytes(&mut nonce);
        let message = ClientHello {
            build_info: Some(BuildInfo {
                product: PRODUCT_CLIENT,
                product_flags: vec![PRODUCT_FLAG_NONE],
                platform: platform(),
                version: SPOTIFY_VERSION,
            }),
            cryptosuites_supported: vec![CRYPTO_SUITE_SHANNON],
            login_crypto_hello: Some(LoginCryptoHelloUnion {
                diffie_hellman: Some(LoginCryptoDiffieHellmanHello {
                    gc: secret.public_key(),
                    server_keys_known: SERVER_KEYS_KNOWN,
                }),
            }),
            client_nonce: nonce,
            padding: Some(vec![PADDING]),
        };
        let mut hello = HELLO_MAGIC.to_vec();
        hello.extend_from_slice(&frame_length(HELLO_MAGIC.len(), &message));
        message
            .encode(&mut hello)
            .expect("a vector grows to fit the hello");
        Handshake { secret, hello }
    }

    #[must_use]
    pub fn hello(&self) -> &[u8] {
        &self.hello
    }

    pub fn response_len(header: [u8; Handshake::RESPONSE_HEADER_LEN]) -> Result<usize> {
        let length = usize::try_from(u32::from_be_bytes(header)).unwrap_or(usize::MAX);
        if length < LENGTH_BYTES {
            return Err(Error::Malformed("access point response length"));
        }
        if length > RESPONSE_LIMIT {
            return Err(Error::Oversized(length));
        }
        Ok(length)
    }

    pub fn finish(self, response: &[u8]) -> Result<Established> {
        let body = response
            .get(LENGTH_BYTES..)
            .ok_or(Error::Malformed("access point response"))?;
        let message =
            ApResponseMessage::decode(body).map_err(Error::decode("access point response"))?;
        let Some(challenge) = message
            .challenge
            .and_then(|challenge| challenge.login_crypto_challenge)
            .and_then(|union| union.diffie_hellman)
        else {
            let code = message
                .login_failed
                .and_then(|failed| failed.error_code)
                .ok_or(Error::Malformed("access point response"))?;
            return Err(Error::LoginFailed(LoginError::from_code(code)));
        };
        let remote = challenge.gs.ok_or(Error::Malformed("access point key"))?;
        let signature = challenge
            .gs_signature
            .ok_or(Error::Malformed("access point key signature"))?;
        if !crate::server_key::verifies(&remote, &signature) {
            return Err(Error::Signature);
        }
        let shared = self.secret.shared_secret(&remote)?;
        let mut exchanged = self.hello;
        exchanged.extend_from_slice(response);
        let derived = derive(&shared, &exchanged);
        let mut reply_mac = mac(&derived[CHALLENGE_KEY]);
        reply_mac.update(&exchanged);
        let reply = ClientResponsePlaintext {
            login_crypto_response: Some(LoginCryptoResponseUnion {
                diffie_hellman: Some(LoginCryptoDiffieHellmanResponse {
                    hmac: reply_mac.finalize().into_bytes().to_vec(),
                }),
            }),
            pow_response: Some(PowResponseUnion {}),
            crypto_response: Some(CryptoResponseUnion {}),
        };
        let mut framed = frame_length(0, &reply).to_vec();
        reply
            .encode(&mut framed)
            .expect("a vector grows to fit the reply");
        let mut keys = Keys {
            send: [0; KEY_LEN],
            receive: [0; KEY_LEN],
        };
        keys.send.copy_from_slice(&derived[SEND_KEY]);
        keys.receive.copy_from_slice(&derived[RECEIVE_KEY]);
        Ok(Established {
            reply: framed,
            keys,
        })
    }
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Keys").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DhSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("DhSecret").finish_non_exhaustive()
    }
}

fn frame_length(prefix: usize, message: &impl prost::Message) -> [u8; LENGTH_BYTES] {
    let total = prefix + LENGTH_BYTES + message.encoded_len();
    u32::try_from(total)
        .expect("handshake messages stay far below 4 GiB")
        .to_be_bytes()
}

fn derive(shared: &[u8], exchanged: &[u8]) -> Vec<u8> {
    let mut derived = Vec::with_capacity(usize::from(DERIVED_ROUNDS) * 20);
    for round in 1..=DERIVED_ROUNDS {
        let mut round_mac = mac(shared);
        round_mac.update(exchanged);
        round_mac.update(&[round]);
        derived.extend_from_slice(&round_mac.finalize().into_bytes());
    }
    derived
}

fn mac(key: &[u8]) -> Hmac<Sha1> {
    <Hmac<Sha1> as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length")
}

fn platform() -> i32 {
    const LINUX_X86: i32 = 0x02;
    const LINUX_X86_64: i32 = 0x08;
    const LINUX_ARM: i32 = 0x11;
    match std::env::consts::ARCH {
        "x86_64" => LINUX_X86_64,
        "arm" | "aarch64" => LINUX_ARM,
        _ => LINUX_X86,
    }
}
