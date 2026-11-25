use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use rikka_secrets::{Entry, Secrets, schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::random;
use crate::secret::SecretText;

const KIND: &str = "kind";
const DEVICE: &str = "device";
const LABEL: &str = "Minecraft device key";
const WINDOWS_EPOCH: i64 = 11_644_473_600;
const TICKS_PER_SECOND: i64 = 10_000_000;

#[derive(Serialize, Deserialize)]
struct Stored {
    id: String,
    key: SecretText,
}

pub(crate) struct DeviceKey {
    id: String,
    signing: SigningKey,
    x: String,
    y: String,
}

impl DeviceKey {
    pub(crate) async fn load() -> Result<DeviceKey> {
        let secrets = Secrets::unlocked().await?;
        let entry = Entry::new(schema::MINECRAFT).attribute(KIND, DEVICE);
        if let Some(stored) = secrets.lookup_json::<Stored>(&entry).await? {
            let scalar = Zeroizing::new(
                rikka_types::hex::decode(&stored.key.0).map_err(|_| Error::DeviceKey)?,
            );
            let signing = SigningKey::from_slice(&scalar).map_err(|_| Error::DeviceKey)?;
            return DeviceKey::new(stored.id, signing);
        }
        let key = DeviceKey::generate()?;
        let stored = Stored {
            id: key.id.clone(),
            key: SecretText(Zeroizing::new(rikka_types::hex::encode(
                key.signing.to_bytes(),
            ))),
        };
        secrets.store_json(&entry, LABEL, &stored).await?;
        Ok(key)
    }

    fn generate() -> Result<DeviceKey> {
        loop {
            let candidate = random::bytes::<32>()?;
            if let Ok(signing) = SigningKey::from_slice(candidate.as_slice()) {
                return DeviceKey::new(random::uuid()?, signing);
            }
        }
    }

    fn new(id: String, signing: SigningKey) -> Result<DeviceKey> {
        let point = signing.verifying_key().to_sec1_point(false);
        let (Some(x), Some(y)) = (point.x(), point.y()) else {
            return Err(Error::DeviceKey);
        };
        Ok(DeviceKey {
            x: URL_SAFE_NO_PAD.encode(x),
            y: URL_SAFE_NO_PAD.encode(y),
            id,
            signing,
        })
    }

    pub(crate) fn id(&self) -> String {
        format!("{{{}}}", self.id.to_uppercase())
    }

    pub(crate) fn proof_key(&self) -> Value {
        json!({
            "kty": "EC",
            "x": self.x,
            "y": self.y,
            "crv": "P-256",
            "alg": "ES256",
            "use": "sig",
        })
    }

    pub(crate) fn sign(&self, path: &str, authorization: &str, body: &[u8], unix: i64) -> String {
        let ticks = ((unix + WINDOWS_EPOCH) * TICKS_PER_SECOND).to_be_bytes();
        let mut message = Vec::with_capacity(body.len() + path.len() + 64);
        message.extend_from_slice(&1u32.to_be_bytes());
        message.push(0);
        message.extend_from_slice(&ticks);
        message.push(0);
        message.extend_from_slice(b"POST");
        message.push(0);
        message.extend_from_slice(path.as_bytes());
        message.push(0);
        message.extend_from_slice(authorization.as_bytes());
        message.push(0);
        message.extend_from_slice(body);
        message.push(0);
        let signature: Signature = self.signing.sign(&message);
        let mut header = Vec::with_capacity(76);
        header.extend_from_slice(&1u32.to_be_bytes());
        header.extend_from_slice(&ticks);
        header.extend_from_slice(&signature.to_bytes());
        STANDARD.encode(header)
    }
}
