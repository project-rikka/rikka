use crate::crypto;
use crate::key::{Credential, State};
use crate::link::Reply;

const REGISTER: u8 = 0x01;
const AUTHENTICATE: u8 = 0x02;
const VERSION: u8 = 0x03;
const CHECK_ONLY: u8 = 0x07;
const ENFORCE_PRESENCE: u8 = 0x03;

const NO_ERROR: [u8; 2] = [0x90, 0x00];
const CONDITIONS_NOT_SATISFIED: [u8; 2] = [0x69, 0x85];
const WRONG_DATA: [u8; 2] = [0x6a, 0x80];
const WRONG_LENGTH: [u8; 2] = [0x67, 0x00];
const INS_NOT_SUPPORTED: [u8; 2] = [0x6d, 0x00];

const CERTIFICATE: [u8; 5] = [0x30, 0x03, 0x02, 0x01, 0x00];

pub(crate) fn handle(state: &mut State, apdu: &[u8]) -> Reply {
    state.apdus.push(apdu.to_vec());

    if apdu.len() < 7 || apdu[0] != 0 || apdu[4] != 0 {
        return Reply::Now(WRONG_LENGTH.to_vec());
    }
    let length = usize::from(u16::from_be_bytes([apdu[5], apdu[6]]));
    let data = if apdu.len() == 7 {
        &[][..]
    } else {
        match apdu.get(7..7 + length) {
            Some(data) => data,
            None => return Reply::Now(WRONG_LENGTH.to_vec()),
        }
    };

    match (apdu[1], apdu[2]) {
        (VERSION, _) => Reply::Now([b"U2F_V2".as_slice(), &NO_ERROR].concat()),
        (AUTHENTICATE, mode) => authenticate(state, mode, data),
        (REGISTER, _) => register(state, data),
        _ => Reply::Now(INS_NOT_SUPPORTED.to_vec()),
    }
}

fn authenticate(state: &mut State, mode: u8, data: &[u8]) -> Reply {
    if data.len() < 65 || data.len() != 65 + usize::from(data[64]) {
        return Reply::Now(WRONG_LENGTH.to_vec());
    }
    let challenge = &data[..32];
    let application: [u8; 32] = data[32..64].try_into().expect("32 bytes");
    let handle = &data[65..];

    let known = state
        .credentials
        .iter()
        .any(|credential| credential.application == application && credential.id == handle);
    if !known {
        return Reply::Now(WRONG_DATA.to_vec());
    }

    match mode {
        CHECK_ONLY => Reply::Now(CONDITIONS_NOT_SATISFIED.to_vec()),
        ENFORCE_PRESENCE if state.waiting < state.touches => {
            state.waiting += 1;
            Reply::Now(CONDITIONS_NOT_SATISFIED.to_vec())
        }
        ENFORCE_PRESENCE => {
            state.waiting = 0;
            state.counter += 1;
            let mut signed = application.to_vec();
            signed.push(0x01);
            signed.extend_from_slice(&state.counter.to_be_bytes());
            signed.extend_from_slice(challenge);
            let signature = crypto::sha256(&signed);

            let mut body = vec![0x01];
            body.extend_from_slice(&state.counter.to_be_bytes());
            body.extend_from_slice(&signature);
            body.extend_from_slice(&NO_ERROR);
            Reply::Now(body)
        }
        _ => Reply::Now(WRONG_DATA.to_vec()),
    }
}

fn register(state: &mut State, data: &[u8]) -> Reply {
    if data.len() != 64 {
        return Reply::Now(WRONG_LENGTH.to_vec());
    }
    if state.waiting < state.touches {
        state.waiting += 1;
        return Reply::Now(CONDITIONS_NOT_SATISFIED.to_vec());
    }
    state.waiting = 0;

    let application: [u8; 32] = data[32..].try_into().expect("32 bytes");
    let handle = crypto::random::<48>().to_vec();
    state
        .credentials
        .push(Credential::new(application, handle.clone(), None));

    let mut body = vec![0x05, 0x04];
    body.extend_from_slice(&[0x07; 64]);
    body.push(handle.len() as u8);
    body.extend_from_slice(&handle);
    body.extend_from_slice(&CERTIFICATE);
    body.extend_from_slice(&crypto::sha256(data));
    body.extend_from_slice(&NO_ERROR);
    Reply::Now(body)
}
