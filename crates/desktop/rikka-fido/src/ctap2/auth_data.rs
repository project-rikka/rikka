use crate::cbor::Value;
use crate::error::{Error, Result};

const HEADER: usize = 37;
const ATTESTED: u8 = 0x40;
const EXTENDED: u8 = 0x80;

pub(crate) struct AuthData {
    pub(crate) rp_id_hash: [u8; 32],
    pub(crate) credential: Option<AttestedCredentialData>,
    pub(crate) extensions: Value,
}

pub(crate) struct AttestedCredentialData {
    pub(crate) id: Vec<u8>,
    pub(crate) public_key: Vec<u8>,
    pub(crate) algorithm: i64,
}

impl AuthData {
    pub(crate) fn parse(bytes: &[u8]) -> Result<AuthData> {
        if bytes.len() < HEADER {
            return Err(Error::Protocol("authenticator data is too short"));
        }

        let rp_id_hash: [u8; 32] = bytes[..32].try_into().expect("header holds 32 bytes");
        let flags = bytes[32];
        let mut rest = &bytes[HEADER..];

        let credential = if flags & ATTESTED == 0 {
            None
        } else {
            let (credential, used) = parse_attested_credential(rest)?;
            rest = &rest[used..];
            Some(credential)
        };

        let extensions = if flags & EXTENDED == 0 {
            Value::Map(Vec::new())
        } else {
            let (extensions, used) = Value::decode_prefix(rest)?;
            if extensions.as_map().is_none() {
                return Err(Error::Protocol("authenticator extensions are not a map"));
            }
            rest = &rest[used..];
            extensions
        };

        if !rest.is_empty() {
            return Err(Error::Protocol("authenticator data has trailing bytes"));
        }

        Ok(AuthData {
            rp_id_hash,
            credential,
            extensions,
        })
    }
}

fn parse_attested_credential(bytes: &[u8]) -> Result<(AttestedCredentialData, usize)> {
    let truncated = || Error::Protocol("attested credential data is truncated");

    let length = bytes.get(16..18).ok_or_else(truncated)?;
    let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
    let id = bytes.get(18..18 + length).ok_or_else(truncated)?.to_vec();
    let key_bytes = &bytes[18 + length..];
    let (key, used) = Value::decode_prefix(key_bytes)?;
    let algorithm = key
        .get(3)
        .and_then(Value::as_i64)
        .ok_or(Error::Protocol("credential public key missing alg"))?;

    Ok((
        AttestedCredentialData {
            id,
            public_key: key_bytes[..used].to_vec(),
            algorithm,
        },
        18 + length + used,
    ))
}
