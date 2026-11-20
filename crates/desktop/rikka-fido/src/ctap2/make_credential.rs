use super::command::{self, MAKE_CREDENTIAL};
use super::get_assertion::{PUBLIC_KEY, descriptors};
use super::pin_protocol::PinProtocol;
use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::ctaphid::Channel;
use crate::error::{Error, Result};
use crate::webauthn::{Algorithm, RelyingParty, User};

pub(crate) enum Auth {
    Signed(Vec<u8>, PinProtocol),
    Blank,
}

pub(crate) struct Make<'a> {
    pub(crate) client_data_hash: &'a [u8; 32],
    pub(crate) rp: &'a RelyingParty,
    pub(crate) user: &'a User,
    pub(crate) algorithms: &'a [Algorithm],
    pub(crate) exclude: &'a [Vec<u8>],
    pub(crate) resident: bool,
    pub(crate) built_in_uv: bool,
    pub(crate) auth: Option<Auth>,
    pub(crate) extensions: Option<Value>,
}

pub(crate) struct Made {
    pub(crate) format: String,
    pub(crate) auth_data: Vec<u8>,
    pub(crate) statement: Value,
    pub(crate) large_blob_key: Option<Vec<u8>>,
}

pub(crate) fn make_credential(
    channel: &mut Channel,
    make: &Make<'_>,
    watch: &Watch,
) -> Result<Made> {
    let mut rp = vec![("id", Value::from(make.rp.id()))];
    if let Some(name) = make.rp.name() {
        rp.push(("name", Value::from(name)));
    }

    let mut user = vec![("id", Value::bytes(make.user.id()))];
    if let Some(name) = make.user.name() {
        user.push(("name", Value::from(name)));
    }
    if let Some(display_name) = make.user.display_name() {
        user.push(("displayName", Value::from(display_name)));
    }

    let parameters = Value::array(make.algorithms.iter().map(|algorithm| {
        Value::map([
            ("alg", Value::from(algorithm.id())),
            ("type", Value::from(PUBLIC_KEY)),
        ])
    }));

    let mut fields = vec![
        (
            Value::from(0x01),
            Value::bytes(make.client_data_hash.to_vec()),
        ),
        (Value::from(0x02), Value::map(rp)),
        (Value::from(0x03), Value::map(user)),
        (Value::from(0x04), parameters),
    ];
    if !make.exclude.is_empty() {
        fields.push((Value::from(0x05), descriptors(make.exclude)));
    }
    if let Some(extensions) = &make.extensions {
        fields.push((Value::from(0x06), extensions.clone()));
    }

    let mut options = Vec::new();
    if make.resident {
        options.push(("rk", true));
    }
    if make.built_in_uv {
        options.push(("uv", true));
    }
    if !options.is_empty() {
        fields.push((Value::from(0x07), Value::map(options)));
    }

    match &make.auth {
        Some(Auth::Signed(param, protocol)) => {
            fields.push((Value::from(0x08), Value::bytes(param.clone())));
            fields.push((Value::from(0x09), Value::from(protocol.number())));
        }
        Some(Auth::Blank) => {
            fields.push((Value::from(0x08), Value::bytes(Vec::new())));
            fields.push((Value::from(0x09), Value::from(PinProtocol::One.number())));
        }
        None => {}
    }

    let reply = command::call(channel, MAKE_CREDENTIAL, Some(&Value::Map(fields)), watch)?;

    Ok(Made {
        format: reply
            .get(0x01)
            .and_then(Value::as_text)
            .ok_or(Error::Protocol("attestation missing fmt"))?
            .to_owned(),
        auth_data: command::bytes(&reply, 0x02, "attestation missing authData")?,
        statement: reply
            .get(0x03)
            .cloned()
            .unwrap_or_else(|| Value::Map(Vec::new())),
        large_blob_key: reply
            .get(0x05)
            .and_then(Value::as_bytes)
            .map(<[u8]>::to_vec),
    })
}
