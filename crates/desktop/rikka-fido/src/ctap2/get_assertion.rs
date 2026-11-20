use super::command::{self, GET_ASSERTION, GET_NEXT_ASSERTION};
use super::pin_protocol::PinProtocol;
use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::ctaphid::Channel;
use crate::error::{Error, Result};
use crate::webauthn::User;

pub(crate) const PUBLIC_KEY: &str = "public-key";

pub(crate) struct Ask<'a> {
    pub(crate) rp_id: &'a str,
    pub(crate) client_data_hash: &'a [u8; 32],
    pub(crate) allow: &'a [Vec<u8>],
    pub(crate) presence: bool,
    pub(crate) built_in_uv: bool,
    pub(crate) auth: Option<(Vec<u8>, PinProtocol)>,
    pub(crate) extensions: Option<Value>,
}

pub(crate) struct Signed {
    pub(crate) credential: Option<Vec<u8>>,
    pub(crate) auth_data: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    pub(crate) user: Option<User>,
    pub(crate) count: u64,
    pub(crate) large_blob_key: Option<Vec<u8>>,
}

pub(crate) fn descriptors(ids: &[Vec<u8>]) -> Value {
    Value::array(ids.iter().map(|id| {
        Value::map([
            ("id", Value::bytes(id.clone())),
            ("type", Value::from(PUBLIC_KEY)),
        ])
    }))
}

pub(crate) fn get_assertion(channel: &mut Channel, ask: &Ask<'_>, watch: &Watch) -> Result<Signed> {
    let mut fields = vec![
        (Value::from(0x01), Value::from(ask.rp_id)),
        (
            Value::from(0x02),
            Value::bytes(ask.client_data_hash.to_vec()),
        ),
    ];
    if !ask.allow.is_empty() {
        fields.push((Value::from(0x03), descriptors(ask.allow)));
    }
    if let Some(extensions) = &ask.extensions {
        fields.push((Value::from(0x04), extensions.clone()));
    }

    let mut options = Vec::new();
    if !ask.presence {
        options.push(("up", false));
    }
    if ask.built_in_uv {
        options.push(("uv", true));
    }
    if !options.is_empty() {
        fields.push((Value::from(0x05), Value::map(options)));
    }

    if let Some((param, protocol)) = &ask.auth {
        fields.push((Value::from(0x06), Value::bytes(param.clone())));
        fields.push((Value::from(0x07), Value::from(protocol.number())));
    }

    let reply = command::call(channel, GET_ASSERTION, Some(&Value::Map(fields)), watch)?;
    signed(&reply)
}

pub(crate) fn get_next_assertion(channel: &mut Channel, watch: &Watch) -> Result<Signed> {
    let reply = command::call(channel, GET_NEXT_ASSERTION, None, watch)?;
    signed(&reply)
}

fn signed(reply: &Value) -> Result<Signed> {
    let credential = reply
        .get(0x01)
        .and_then(|descriptor| descriptor.get("id"))
        .and_then(Value::as_bytes)
        .map(<[u8]>::to_vec);

    let user = reply.get(0x04).map(user).transpose()?;

    Ok(Signed {
        credential,
        auth_data: command::bytes(reply, 0x02, "assertion missing authData")?,
        signature: command::bytes(reply, 0x03, "assertion missing signature")?,
        user,
        count: reply.get(0x05).and_then(Value::as_unsigned).unwrap_or(1),
        large_blob_key: reply
            .get(0x07)
            .and_then(Value::as_bytes)
            .map(<[u8]>::to_vec),
    })
}

fn user(entity: &Value) -> Result<User> {
    let id = entity
        .get("id")
        .and_then(Value::as_bytes)
        .ok_or(Error::Protocol("assertion user missing id"))?;
    let mut user = User::new(id);
    if let Some(name) = entity.get("name").and_then(Value::as_text) {
        user = user.with_name(name);
    }
    if let Some(display_name) = entity.get("displayName").and_then(Value::as_text) {
        user = user.with_display_name(display_name);
    }
    Ok(user)
}
