use rikka_fido::User;
use rikka_fido::cbor::Value;

use crate::crypto;
use crate::key::{Credential, Kind, State};
use crate::link::Reply;

const MAKE_CREDENTIAL: u8 = 0x01;
const GET_ASSERTION: u8 = 0x02;
const GET_INFO: u8 = 0x04;
const CLIENT_PIN: u8 = 0x06;
const GET_NEXT_ASSERTION: u8 = 0x08;
const SELECTION: u8 = 0x0b;
const LARGE_BLOBS: u8 = 0x0c;

const OK: u8 = 0x00;
const INVALID_COMMAND: u8 = 0x01;
const INVALID_PARAMETER: u8 = 0x02;
const INVALID_CBOR: u8 = 0x12;
const MISSING_PARAMETER: u8 = 0x14;
const CREDENTIAL_EXCLUDED: u8 = 0x19;
const UNSUPPORTED_ALGORITHM: u8 = 0x26;
const NO_CREDENTIALS: u8 = 0x2e;
const NOT_ALLOWED: u8 = 0x30;
const PIN_INVALID: u8 = 0x31;
const PIN_BLOCKED: u8 = 0x32;
const PIN_AUTH_INVALID: u8 = 0x33;
const PIN_NOT_SET: u8 = 0x35;
const PUAT_REQUIRED: u8 = 0x36;
const PIN_POLICY_VIOLATION: u8 = 0x37;
const INVALID_SUBCOMMAND: u8 = 0x3e;

const AAGUID: [u8; 16] = [0x11; 16];
const UP: u8 = 0x01;
const UV: u8 = 0x04;
const AT: u8 = 0x40;
const ED: u8 = 0x80;
const LARGE_BLOB_ARRAY: u64 = 8192;
const MAX_MESSAGE: u64 = 1200;

pub(crate) fn handle(state: &mut State, data: &[u8]) -> Reply {
    let Some((&command, body)) = data.split_first() else {
        return status(INVALID_COMMAND);
    };
    let parameters = if body.is_empty() {
        Value::Map(Vec::new())
    } else {
        match Value::decode(body) {
            Ok(parameters) => parameters,
            Err(_) => return status(INVALID_CBOR),
        }
    };
    state.requests.push((command, parameters.clone()));

    match command {
        GET_INFO => answer(info(state)),
        CLIENT_PIN => client_pin(state, &parameters),
        GET_ASSERTION => get_assertion(state, &parameters),
        GET_NEXT_ASSERTION => match state.next.pop() {
            Some(reply) => answer(reply),
            None => status(NOT_ALLOWED),
        },
        MAKE_CREDENTIAL => make_credential(state, &parameters),
        SELECTION if state.kind == Kind::Ctap21 => Reply::AfterTouch(vec![OK]),
        LARGE_BLOBS if state.kind == Kind::Ctap21 => large_blobs(state, &parameters),
        _ => status(INVALID_COMMAND),
    }
}

fn info(state: &State) -> Value {
    let (versions, extensions) = match state.kind {
        Kind::Ctap21 => (
            vec!["U2F_V2", "FIDO_2_0", "FIDO_2_1"],
            vec![
                "credProtect",
                "hmac-secret",
                "hmac-secret-mc",
                "largeBlobKey",
                "minPinLength",
            ],
        ),
        Kind::Ctap20 | Kind::U2f => (vec!["U2F_V2", "FIDO_2_0"], vec!["hmac-secret"]),
    };

    let mut options = vec![("rk", true), ("up", true)];
    if state.pin_supported {
        options.push(("clientPin", state.pin.is_some()));
    }
    if state.kind == Kind::Ctap21 {
        options.push(("largeBlobs", true));
        if state.pin_supported {
            options.push(("pinUvAuthToken", true));
        }
    }

    let mut fields = vec![
        (Value::from(0x01), Value::array(versions)),
        (Value::from(0x02), Value::array(extensions)),
        (Value::from(0x03), Value::bytes(AAGUID)),
        (Value::from(0x04), Value::map(options)),
        (Value::from(0x05), Value::from(MAX_MESSAGE)),
        (Value::from(0x08), Value::from(128)),
    ];
    if state.pin_supported {
        fields.push((Value::from(0x06), Value::array(state.protocols.clone())));
    }
    if let Some(count) = state.max_credentials {
        fields.push((Value::from(0x07), Value::from(count)));
    }
    if state.kind == Kind::Ctap21 {
        fields.push((Value::from(0x0b), Value::from(LARGE_BLOB_ARRAY)));
        fields.push((Value::from(0x0c), Value::Bool(state.force_pin_change)));
        fields.push((Value::from(0x0d), Value::from(state.min_pin_length)));
    }
    Value::Map(fields)
}

fn client_pin(state: &mut State, parameters: &Value) -> Reply {
    let Some(protocol) = parameters.get(0x01).and_then(Value::as_unsigned) else {
        return status(MISSING_PARAMETER);
    };
    if !state.protocols.contains(&protocol) {
        return status(INVALID_PARAMETER);
    }

    match parameters.get(0x02).and_then(Value::as_unsigned) {
        Some(0x01) => answer(Value::map([(0x03, Value::from(state.retries))])),
        Some(0x02) => {
            let key = state.agreement.get_or_insert_with(crypto::agreement_key);
            answer(Value::map([(0x01, crypto::cose(key))]))
        }
        Some(0x03) => set_pin(state, protocol, parameters),
        Some(0x04) => change_pin(state, protocol, parameters),
        Some(subcommand @ (0x05 | 0x09)) => pin_token(state, protocol, subcommand, parameters),
        _ => status(INVALID_SUBCOMMAND),
    }
}

fn shared(state: &State, protocol: u64, parameters: &Value) -> Result<Vec<u8>, u8> {
    let peer = parameters.get(0x03).ok_or(MISSING_PARAMETER)?;
    let key = state.agreement.as_ref().ok_or(MISSING_PARAMETER)?;
    crypto::shared_secret(protocol, key, peer).ok_or(INVALID_PARAMETER)
}

fn accepted_new_pin(
    state: &State,
    protocol: u64,
    secret: &[u8],
    sealed: &[u8],
) -> Result<String, u8> {
    let padded = crypto::decrypt(protocol, secret, sealed).ok_or(INVALID_PARAMETER)?;
    if padded.len() != 64 {
        return Err(INVALID_PARAMETER);
    }
    let end = padded.iter().position(|byte| *byte == 0).unwrap_or(64);
    let pin = String::from_utf8(padded[..end].to_vec()).map_err(|_| PIN_POLICY_VIOLATION)?;
    if (pin.chars().count() as u64) < state.min_pin_length || pin.len() > 63 {
        return Err(PIN_POLICY_VIOLATION);
    }
    Ok(pin)
}

fn set_pin(state: &mut State, protocol: u64, parameters: &Value) -> Reply {
    if state.pin.is_some() {
        return status(NOT_ALLOWED);
    }
    let secret = match shared(state, protocol, parameters) {
        Ok(secret) => secret,
        Err(code) => return status(code),
    };
    let (Some(param), Some(sealed)) = (
        parameters.get(0x04).and_then(Value::as_bytes),
        parameters.get(0x05).and_then(Value::as_bytes),
    ) else {
        return status(MISSING_PARAMETER);
    };
    if !crypto::verify(protocol, &secret, sealed, param) {
        return status(PIN_AUTH_INVALID);
    }
    match accepted_new_pin(state, protocol, &secret, sealed) {
        Ok(pin) => {
            state.pin = Some(pin);
            state.retries = 8;
            answer(Value::Map(Vec::new()))
        }
        Err(code) => status(code),
    }
}

fn change_pin(state: &mut State, protocol: u64, parameters: &Value) -> Reply {
    let Some(current) = state.pin.clone() else {
        return status(PIN_NOT_SET);
    };
    if state.retries == 0 {
        return status(PIN_BLOCKED);
    }
    let secret = match shared(state, protocol, parameters) {
        Ok(secret) => secret,
        Err(code) => return status(code),
    };
    let (Some(param), Some(sealed), Some(hash)) = (
        parameters.get(0x04).and_then(Value::as_bytes),
        parameters.get(0x05).and_then(Value::as_bytes),
        parameters.get(0x06).and_then(Value::as_bytes),
    ) else {
        return status(MISSING_PARAMETER);
    };
    if !crypto::verify(protocol, &secret, &[sealed, hash].concat(), param) {
        return status(PIN_AUTH_INVALID);
    }
    if let Err(code) = check_pin_hash(state, protocol, &secret, hash, &current) {
        return status(code);
    }
    match accepted_new_pin(state, protocol, &secret, sealed) {
        Ok(pin) => {
            state.pin = Some(pin);
            state.force_pin_change = false;
            answer(Value::Map(Vec::new()))
        }
        Err(code) => status(code),
    }
}

fn check_pin_hash(
    state: &mut State,
    protocol: u64,
    secret: &[u8],
    sealed: &[u8],
    pin: &str,
) -> Result<(), u8> {
    let expected = &crypto::sha256(pin.as_bytes())[..16];
    if crypto::decrypt(protocol, secret, sealed).as_deref() == Some(expected) {
        state.retries = 8;
        return Ok(());
    }
    state.retries -= 1;
    state.agreement = Some(crypto::agreement_key());
    Err(if state.retries == 0 {
        PIN_BLOCKED
    } else {
        PIN_INVALID
    })
}

fn pin_token(state: &mut State, protocol: u64, subcommand: u64, parameters: &Value) -> Reply {
    let Some(pin) = state.pin.clone() else {
        return status(PIN_NOT_SET);
    };
    if state.retries == 0 {
        return status(PIN_BLOCKED);
    }
    let secret = match shared(state, protocol, parameters) {
        Ok(secret) => secret,
        Err(code) => return status(code),
    };
    let Some(sealed) = parameters.get(0x06).and_then(Value::as_bytes) else {
        return status(MISSING_PARAMETER);
    };
    if let Err(code) = check_pin_hash(state, protocol, &secret, sealed, &pin) {
        return status(code);
    }
    if state.force_pin_change {
        return status(PIN_POLICY_VIOLATION);
    }

    if subcommand == 0x09 {
        let permissions = parameters
            .get(0x09)
            .and_then(Value::as_unsigned)
            .unwrap_or_default();
        let rp_id = parameters
            .get(0x0a)
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_owned();
        state.permissions.push((permissions, rp_id));
    }

    let token = crypto::random::<32>().to_vec();
    let sealed = crypto::encrypt(protocol, &secret, &token);
    state.token = Some(token);
    answer(Value::map([(0x02, Value::bytes(sealed))]))
}

fn verified(
    state: &State,
    parameters: &Value,
    message_key: u64,
    param_key: u64,
) -> Result<bool, u8> {
    let Some(param) = parameters.get(param_key).and_then(Value::as_bytes) else {
        return if state.pin.is_some() && param_key == 0x08 {
            Err(PUAT_REQUIRED)
        } else {
            Ok(false)
        };
    };
    let protocol = parameters
        .get(param_key + 1)
        .and_then(Value::as_unsigned)
        .unwrap_or(1);
    let message = parameters
        .get(message_key)
        .and_then(Value::as_bytes)
        .unwrap_or_default();
    match &state.token {
        Some(token) if crypto::verify(protocol, token, message, param) => Ok(true),
        _ => Err(PIN_AUTH_INVALID),
    }
}

fn hmac_secret(
    state: &State,
    input: &Value,
    credential: &Credential,
    uv: bool,
) -> Result<Vec<u8>, u8> {
    let protocol = input.get(0x04).and_then(Value::as_unsigned).unwrap_or(1);
    let peer = input.get(0x01).ok_or(MISSING_PARAMETER)?;
    let key = state.agreement.as_ref().ok_or(MISSING_PARAMETER)?;
    let secret = crypto::shared_secret(protocol, key, peer).ok_or(INVALID_PARAMETER)?;
    let (Some(salt_enc), Some(salt_auth)) = (
        input.get(0x02).and_then(Value::as_bytes),
        input.get(0x03).and_then(Value::as_bytes),
    ) else {
        return Err(MISSING_PARAMETER);
    };
    if !crypto::verify(protocol, &secret, salt_enc, salt_auth) {
        return Err(PIN_AUTH_INVALID);
    }
    let salts = crypto::decrypt(protocol, &secret, salt_enc).ok_or(INVALID_PARAMETER)?;
    if salts.len() != 32 && salts.len() != 64 {
        return Err(INVALID_PARAMETER);
    }

    let random = if uv {
        credential.cred_random_uv
    } else {
        credential.cred_random
    };
    let outputs: Vec<u8> = salts
        .chunks(32)
        .flat_map(|salt| crypto::hmac(&random, salt))
        .collect();
    Ok(crypto::encrypt(protocol, &secret, &outputs))
}

fn get_assertion(state: &mut State, parameters: &Value) -> Reply {
    let (Some(rp_id), Some(hash)) = (
        parameters.get(0x01).and_then(Value::as_text),
        parameters.get(0x02).and_then(Value::as_bytes),
    ) else {
        return status(MISSING_PARAMETER);
    };
    let uv = match verified(state, parameters, 0x02, 0x06) {
        Ok(uv) => uv,
        Err(code) => return status(code),
    };
    let presence = parameters
        .get(0x05)
        .and_then(|options| options.get("up"))
        .and_then(Value::as_bool)
        != Some(false);
    let extensions = parameters.get(0x04);

    let application = crypto::sha256(rp_id);
    let allowed: Vec<Vec<u8>> = parameters
        .get(0x03)
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|descriptor| descriptor.get("id")?.as_bytes().map(<[u8]>::to_vec))
        .collect();
    let discoverable = allowed.is_empty();

    let matching: Vec<Credential> = state
        .credentials
        .iter()
        .filter(|credential| credential.application == application)
        .filter(|credential| match credential.protection {
            3 => uv,
            2 => uv || !discoverable,
            _ => true,
        })
        .filter(|credential| {
            if discoverable {
                credential.user.is_some()
            } else {
                allowed.contains(&credential.id)
            }
        })
        .cloned()
        .collect();

    if matching.is_empty() {
        return if presence {
            Reply::AfterTouch(vec![NO_CREDENTIALS])
        } else {
            status(NO_CREDENTIALS)
        };
    }

    let mut replies = Vec::new();
    for credential in &matching {
        let mut outputs = Vec::new();
        if let Some(input) = extensions.and_then(|extensions| extensions.get("hmac-secret")) {
            match hmac_secret(state, input, credential, uv) {
                Ok(sealed) => outputs.push((Value::from("hmac-secret"), Value::bytes(sealed))),
                Err(code) => return status(code),
            }
        }

        state.counter += 1;
        let flags = if presence { UP } else { 0 } | if uv { UV } else { 0 };
        let auth_data = authenticator_data(application, flags, state.counter, None, &outputs);
        let signature = crypto::sha256([auth_data.as_slice(), hash].concat()).to_vec();

        let mut fields = vec![
            (
                Value::from(0x01),
                Value::map([
                    ("id", Value::bytes(credential.id.clone())),
                    ("type", Value::from("public-key")),
                ]),
            ),
            (Value::from(0x02), Value::bytes(auth_data)),
            (Value::from(0x03), Value::bytes(signature)),
        ];
        if discoverable && let Some(user) = &credential.user {
            fields.push((Value::from(0x04), user_entity(user)));
        }
        let wants_blob_key = extensions
            .and_then(|extensions| extensions.get("largeBlobKey"))
            .and_then(Value::as_bool)
            == Some(true);
        if wants_blob_key && let Some(key) = credential.large_blob_key {
            fields.push((Value::from(0x07), Value::bytes(key)));
        }
        replies.push(Value::Map(fields));
    }

    if discoverable
        && replies.len() > 1
        && let Value::Map(fields) = &mut replies[0]
    {
        fields.push((Value::from(0x05), Value::from(matching.len())));
    }

    let first = replies.remove(0);
    replies.reverse();
    state.next = replies;

    let bytes = [&[OK][..], &first.encode()].concat();
    if presence {
        Reply::AfterTouch(bytes)
    } else {
        Reply::Now(bytes)
    }
}

fn make_credential(state: &mut State, parameters: &Value) -> Reply {
    let blank = parameters
        .get(0x08)
        .and_then(Value::as_bytes)
        .is_some_and(<[u8]>::is_empty);
    if blank {
        return Reply::AfterTouch(vec![if state.pin.is_some() {
            PIN_INVALID
        } else {
            PIN_NOT_SET
        }]);
    }

    let (Some(rp_id), Some(user), Some(algorithms)) = (
        parameters
            .get(0x02)
            .and_then(|rp| rp.get("id"))
            .and_then(Value::as_text),
        parameters.get(0x03),
        parameters.get(0x04).and_then(Value::as_array),
    ) else {
        return status(MISSING_PARAMETER);
    };

    let uv = match verified(state, parameters, 0x01, 0x08) {
        Ok(uv) => uv,
        Err(code) => return status(code),
    };
    if !algorithms
        .iter()
        .any(|entry| entry.get("alg").and_then(Value::as_i64) == Some(-7))
    {
        return status(UNSUPPORTED_ALGORITHM);
    }

    let application = crypto::sha256(rp_id);
    let excluded = parameters
        .get(0x05)
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|descriptor| descriptor.get("id")?.as_bytes())
        .any(|id| {
            state
                .credentials
                .iter()
                .any(|credential| credential.application == application && credential.id == id)
        });
    if excluded {
        return Reply::AfterTouch(vec![CREDENTIAL_EXCLUDED]);
    }

    let resident = parameters
        .get(0x07)
        .and_then(|options| options.get("rk"))
        .and_then(Value::as_bool)
        == Some(true);
    let extensions = parameters.get(0x06);
    let extension = |name: &str| extensions.and_then(|extensions| extensions.get(name));

    let id = crypto::random::<16>().to_vec();
    let user_id = user.get("id").and_then(Value::as_bytes).unwrap_or_default();
    let mut credential = Credential::new(
        application,
        id.clone(),
        resident.then(|| User::new(user_id)),
    );
    let wants_blob_key = extension("largeBlobKey").and_then(Value::as_bool) == Some(true);
    if !resident || !wants_blob_key {
        credential.large_blob_key = None;
    }

    let mut outputs = Vec::new();
    if let Some(level) = extension("credProtect").and_then(Value::as_unsigned) {
        credential.protection = level;
        outputs.push((Value::from("credProtect"), Value::from(level)));
    }
    if extension("hmac-secret").and_then(Value::as_bool) == Some(true) {
        outputs.push((Value::from("hmac-secret"), Value::Bool(true)));
    }
    if let Some(input) = extension("hmac-secret-mc") {
        match hmac_secret(state, input, &credential, uv) {
            Ok(sealed) => outputs.push((Value::from("hmac-secret-mc"), Value::bytes(sealed))),
            Err(code) => return status(code),
        }
    }
    if extension("minPinLength").and_then(Value::as_bool) == Some(true) {
        outputs.push((
            Value::from("minPinLength"),
            Value::from(state.min_pin_length),
        ));
    }

    let large_blob_key = credential.large_blob_key;
    state.credentials.push(credential);

    let public_key = Value::map([
        (Value::from(1), Value::from(2)),
        (Value::from(3), Value::from(-7)),
        (Value::from(-1), Value::from(1)),
        (Value::from(-2), Value::bytes([0x01; 32])),
        (Value::from(-3), Value::bytes([0x02; 32])),
    ]);
    let flags = UP | AT | if uv { UV } else { 0 };
    let auth_data = authenticator_data(application, flags, 0, Some((&id, &public_key)), &outputs);

    let mut reply = vec![
        (Value::from(0x01), Value::from("none")),
        (Value::from(0x02), Value::bytes(auth_data)),
        (Value::from(0x03), Value::Map(Vec::new())),
    ];
    if let Some(key) = large_blob_key {
        reply.push((Value::from(0x05), Value::bytes(key)));
    }
    Reply::AfterTouch([&[OK][..], &Value::Map(reply).encode()].concat())
}

fn authenticator_data(
    application: [u8; 32],
    flags: u8,
    counter: u32,
    attested: Option<(&[u8], &Value)>,
    extensions: &[(Value, Value)],
) -> Vec<u8> {
    let mut data = application.to_vec();
    data.push(flags | if extensions.is_empty() { 0 } else { ED });
    data.extend_from_slice(&counter.to_be_bytes());
    if let Some((id, public_key)) = attested {
        data.extend_from_slice(&AAGUID);
        data.extend_from_slice(&(id.len() as u16).to_be_bytes());
        data.extend_from_slice(id);
        data.extend_from_slice(&public_key.encode());
    }
    if !extensions.is_empty() {
        data.extend_from_slice(&Value::Map(extensions.to_vec()).encode());
    }
    data
}

fn large_blobs(state: &State, parameters: &Value) -> Reply {
    let (Some(length), Some(offset)) = (
        parameters.get(0x01).and_then(Value::as_unsigned),
        parameters.get(0x03).and_then(Value::as_unsigned),
    ) else {
        return status(MISSING_PARAMETER);
    };
    if length > MAX_MESSAGE - 64 {
        return status(0x03);
    }
    let start = (offset as usize).min(state.large_blobs.len());
    let end = (start + length as usize).min(state.large_blobs.len());
    answer(Value::map([(
        0x01,
        Value::bytes(state.large_blobs[start..end].to_vec()),
    )]))
}

fn user_entity(user: &User) -> Value {
    let mut fields = vec![("id", Value::bytes(user.id()))];
    if let Some(name) = user.name() {
        fields.push(("name", Value::from(name)));
    }
    if let Some(display_name) = user.display_name() {
        fields.push(("displayName", Value::from(display_name)));
    }
    Value::map(fields)
}

fn answer(reply: Value) -> Reply {
    Reply::Now([&[OK][..], &reply.encode()].concat())
}

fn status(code: u8) -> Reply {
    Reply::Now(vec![code])
}
