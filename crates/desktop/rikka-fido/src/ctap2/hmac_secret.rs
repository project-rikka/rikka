use super::client_pin::key_agreement;
use super::pin_protocol::{PinProtocol, SharedSecret};
use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::ctaphid::Channel;
use crate::error::{Error, Result};
use crate::webauthn::{PrfValues, Salts};

pub(crate) const HMAC_SECRET: &str = "hmac-secret";
pub(crate) const HMAC_SECRET_MC: &str = "hmac-secret-mc";

pub(crate) struct SecretAsk {
    secret: SharedSecret,
    input: Value,
    pairs: bool,
}

impl SecretAsk {
    pub(crate) fn prepare(
        channel: &mut Channel,
        protocol: PinProtocol,
        salts: &Salts,
        watch: &Watch,
    ) -> Result<SecretAsk> {
        let (platform, secret) = key_agreement(channel, protocol, watch)?;
        let salt_enc = secret.encrypt(&salts.joined())?;
        let salt_auth = secret.authenticate(&salt_enc);

        let mut fields = vec![
            (Value::from(0x01), platform),
            (Value::from(0x02), Value::Bytes(salt_enc)),
            (Value::from(0x03), Value::Bytes(salt_auth)),
        ];
        if protocol != PinProtocol::One {
            fields.push((Value::from(0x04), Value::from(protocol.number())));
        }

        Ok(SecretAsk {
            secret,
            input: Value::Map(fields),
            pairs: salts.second.is_some(),
        })
    }

    pub(crate) fn input(&self) -> Value {
        self.input.clone()
    }

    pub(crate) fn open(&self, sealed: &Value) -> Result<PrfValues> {
        let sealed = sealed
            .as_bytes()
            .ok_or(Error::Protocol("hmac-secret output is not bytes"))?;
        let outputs = self.secret.decrypt(sealed)?;
        let expected = if self.pairs { 64 } else { 32 };
        if outputs.len() != expected {
            return Err(Error::Protocol("invalid hmac-secret output length"));
        }
        Ok(PrfValues::from_outputs(&outputs))
    }
}
