use music_spotify_api::{
    CLIENT_ID, ClientTokenExchange, Error, Login5Exchange, Platform, Progress,
};
use music_spotify_wire::{DeviceId, StoredCredentials};
use prost::Message;
use sha1::{Digest, Sha1};

const GRANTED: &[u8] = include_bytes!("fixtures/client-token-granted.bin");
const LOGIN5_REFUSED: &[u8] = include_bytes!("fixtures/login5-invalid-credentials.bin");
const ZERO_BITS: i32 = 10;
const STATE: &str = "challenge-state";
const PREFIX: [u8; 16] = [
    0x8f, 0x13, 0x55, 0x02, 0xc4, 0x9a, 0x6e, 0x21, 0x0b, 0x77, 0xd3, 0x40, 0x19, 0xe8, 0x5c, 0xa6,
];
const LOGIN_CONTEXT: &[u8] = b"login-context";

#[derive(Clone, PartialEq, Message)]
struct ClientTokenRequest {
    #[prost(int32, tag = "1")]
    request_type: i32,
    #[prost(message, optional, tag = "2")]
    client_data: Option<ClientData>,
    #[prost(message, optional, tag = "3")]
    challenge_answers: Option<ChallengeAnswers>,
}

#[derive(Clone, PartialEq, Message)]
struct ClientData {
    #[prost(string, tag = "2")]
    client_id: String,
    #[prost(message, optional, tag = "3")]
    connectivity: Option<Connectivity>,
}

#[derive(Clone, PartialEq, Message)]
struct Connectivity {
    #[prost(string, tag = "2")]
    device_id: String,
}

#[derive(Clone, PartialEq, Message)]
struct ChallengeAnswers {
    #[prost(string, tag = "1")]
    state: String,
    #[prost(message, repeated, tag = "2")]
    answers: Vec<ChallengeAnswer>,
}

#[derive(Clone, PartialEq, Message)]
struct ChallengeAnswer {
    #[prost(int32, tag = "1")]
    challenge_type: i32,
    #[prost(message, optional, tag = "4")]
    hash_cash: Option<HashCashAnswer>,
}

#[derive(Clone, PartialEq, Message)]
struct HashCashAnswer {
    #[prost(string, tag = "1")]
    suffix: String,
}

#[derive(Clone, PartialEq, Message)]
struct ClientTokenChallenges {
    #[prost(int32, tag = "1")]
    response_type: i32,
    #[prost(message, optional, tag = "3")]
    challenges: Option<Challenges>,
}

#[derive(Clone, PartialEq, Message)]
struct Challenges {
    #[prost(string, tag = "1")]
    state: String,
    #[prost(message, repeated, tag = "2")]
    challenges: Vec<Challenge>,
}

#[derive(Clone, PartialEq, Message)]
struct Challenge {
    #[prost(int32, tag = "1")]
    challenge_type: i32,
    #[prost(message, optional, tag = "4")]
    hashcash: Option<HashCashParameters>,
}

#[derive(Clone, PartialEq, Message)]
struct HashCashParameters {
    #[prost(int32, tag = "1")]
    length: i32,
    #[prost(string, tag = "2")]
    prefix: String,
}

#[derive(Clone, PartialEq, Message)]
struct LoginRequest {
    #[prost(bytes = "vec", tag = "2")]
    login_context: Vec<u8>,
    #[prost(message, optional, tag = "3")]
    solutions: Option<LoginSolutions>,
    #[prost(message, optional, tag = "100")]
    stored_credential: Option<LoginCredential>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginCredential {
    #[prost(string, tag = "1")]
    username: String,
    #[prost(bytes = "vec", tag = "2")]
    data: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginSolutions {
    #[prost(message, repeated, tag = "1")]
    solutions: Vec<LoginSolution>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginSolution {
    #[prost(message, optional, tag = "1")]
    hashcash: Option<HashcashSolution>,
}

#[derive(Clone, PartialEq, Message)]
struct HashcashSolution {
    #[prost(bytes = "vec", tag = "1")]
    suffix: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginResponse {
    #[prost(message, optional, tag = "1")]
    ok: Option<LoginOk>,
    #[prost(message, optional, tag = "3")]
    challenges: Option<LoginChallenges>,
    #[prost(bytes = "vec", tag = "5")]
    login_context: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginOk {
    #[prost(string, tag = "2")]
    access_token: String,
    #[prost(int32, tag = "4")]
    access_token_expires_in: i32,
}

#[derive(Clone, PartialEq, Message)]
struct LoginChallenges {
    #[prost(message, repeated, tag = "1")]
    challenges: Vec<LoginChallenge>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginChallenge {
    #[prost(message, optional, tag = "1")]
    hashcash: Option<LoginHashcash>,
}

#[derive(Clone, PartialEq, Message)]
struct LoginHashcash {
    #[prost(bytes = "vec", tag = "1")]
    prefix: Vec<u8>,
    #[prost(int32, tag = "2")]
    length: i32,
}

fn device() -> DeviceId {
    DeviceId::generate(&mut rand::rng())
}

fn platform() -> Platform {
    Platform {
        system_name: "Linux".to_owned(),
        system_release: "6.18.55".to_owned(),
        system_version: "26.11".to_owned(),
        hardware: "x86_64".to_owned(),
    }
}

fn credentials() -> StoredCredentials {
    StoredCredentials {
        username: "listener".to_owned(),
        auth_data: vec![0x5a; 32],
    }
}

fn meets_challenge(prefix: &[u8], suffix: &[u8]) -> bool {
    let digest = Sha1::new()
        .chain_update(prefix)
        .chain_update(suffix)
        .finalize();
    let tail = u64::from_be_bytes(digest[12..20].try_into().unwrap());
    tail.trailing_zeros() >= ZERO_BITS.unsigned_abs()
}

#[test]
fn client_token_is_granted_from_recorded_answer() {
    let device = device();
    let mut exchange = ClientTokenExchange::new(&device, &platform());
    let request = ClientTokenRequest::decode(exchange.request().as_slice()).unwrap();
    let data = request.client_data.unwrap();
    assert_eq!(data.client_id, CLIENT_ID);
    assert_eq!(data.connectivity.unwrap().device_id, device.as_str());

    let Progress::Granted(token) = exchange.answer(GRANTED).unwrap() else {
        panic!("a granted answer grants a token");
    };
    assert!(token.value().starts_with("AAHredacted-client-token-"));
    assert!(token.is_fresh());
}

#[test]
fn client_token_challenge_is_answered_with_valid_hashcash() {
    let mut exchange = ClientTokenExchange::new(&device(), &platform());
    let challenge = ClientTokenChallenges {
        response_type: 2,
        challenges: Some(Challenges {
            state: STATE.to_owned(),
            challenges: vec![Challenge {
                challenge_type: 3,
                hashcash: Some(HashCashParameters {
                    length: ZERO_BITS,
                    prefix: rikka_types::hex::encode(PREFIX).to_ascii_uppercase(),
                }),
            }],
        }),
    };
    assert!(matches!(
        exchange.answer(&challenge.encode_to_vec()).unwrap(),
        Progress::Challenged
    ));

    let request = ClientTokenRequest::decode(exchange.request().as_slice()).unwrap();
    assert_eq!(request.request_type, 2);
    let answers = request.challenge_answers.unwrap();
    assert_eq!(answers.state, STATE);
    let suffix = &answers.answers[0].hash_cash.as_ref().unwrap().suffix;
    assert_eq!(suffix, &suffix.to_ascii_uppercase());
    assert!(meets_challenge(
        &PREFIX,
        &rikka_types::hex::decode(suffix).unwrap()
    ));
}

#[test]
fn login5_refusal_is_reported() {
    let mut exchange = Login5Exchange::new(&device(), &credentials());
    assert!(matches!(
        exchange.answer(LOGIN5_REFUSED),
        Err(Error::TokenRefused { .. })
    ));
}

#[test]
fn login5_challenge_is_answered_then_granted() {
    let mut exchange = Login5Exchange::new(&device(), &credentials());
    let first = LoginRequest::decode(exchange.request().as_slice()).unwrap();
    assert_eq!(
        first.stored_credential,
        Some(LoginCredential {
            username: "listener".to_owned(),
            data: vec![0x5a; 32],
        })
    );

    let challenge = LoginResponse {
        ok: None,
        challenges: Some(LoginChallenges {
            challenges: vec![LoginChallenge {
                hashcash: Some(LoginHashcash {
                    prefix: PREFIX.to_vec(),
                    length: ZERO_BITS,
                }),
            }],
        }),
        login_context: LOGIN_CONTEXT.to_vec(),
    };
    assert!(matches!(
        exchange.answer(&challenge.encode_to_vec()).unwrap(),
        Progress::Challenged
    ));
    let answered = LoginRequest::decode(exchange.request().as_slice()).unwrap();
    assert_eq!(answered.login_context, LOGIN_CONTEXT);
    let solutions = answered.solutions.unwrap().solutions;
    let suffix = &solutions[0].hashcash.as_ref().unwrap().suffix;
    assert!(meets_challenge(&PREFIX, suffix));

    let granted = LoginResponse {
        ok: Some(LoginOk {
            access_token: "access".to_owned(),
            access_token_expires_in: 3600,
        }),
        challenges: None,
        login_context: Vec::new(),
    };
    let Progress::Granted(token) = exchange.answer(&granted.encode_to_vec()).unwrap() else {
        panic!("an ok answer grants a token");
    };
    assert_eq!(token.value(), "access");
}
