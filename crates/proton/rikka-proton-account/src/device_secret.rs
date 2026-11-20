use std::fmt;

use rikka_proton_api::base32::{self, Alphabet};
use rikka_proton_pgp::{Secret, SecretBytes, SymmetricKey, random_bytes, sha256};
use serde::{Deserialize, Serialize};

use crate::auth_device::AuthDeviceId;
use crate::error::{Error, Result};

const SECRET_LEN: usize = 32;
const SECRET_CONTEXT: &[u8] = b"account.device-secret";
const CONFIRMATION_CODE_LEN: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ConfirmationCodeVersion {
    #[default]
    HexDigest,
    Digest,
}

impl ConfirmationCodeVersion {
    pub(crate) fn for_device(
        device: &AuthDeviceId,
        version: u8,
    ) -> Result<ConfirmationCodeVersion> {
        match version {
            1 => Ok(ConfirmationCodeVersion::HexDigest),
            2 => Ok(ConfirmationCodeVersion::Digest),
            version => Err(Error::UnknownConfirmationCodeVersion {
                device: device.clone(),
                version,
            }),
        }
    }

    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            ConfirmationCodeVersion::HexDigest => 1,
            ConfirmationCodeVersion::Digest => 2,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSecret {
    #[serde(rename = "DeviceID")]
    device: AuthDeviceId,
    #[serde(rename = "DeviceToken")]
    token: Secret,
    #[serde(rename = "Secret")]
    secret: Secret,
}

impl DeviceSecret {
    #[must_use]
    pub fn new(
        device: AuthDeviceId,
        token: impl Into<String>,
        secret: impl Into<String>,
    ) -> DeviceSecret {
        DeviceSecret {
            device,
            token: Secret::new(token),
            secret: Secret::new(secret),
        }
    }

    #[must_use]
    pub fn device(&self) -> &AuthDeviceId {
        &self.device
    }

    #[must_use]
    pub fn token(&self) -> &Secret {
        &self.token
    }

    #[must_use]
    pub fn secret(&self) -> &Secret {
        &self.secret
    }

    #[must_use]
    pub fn confirmation_code(&self, version: ConfirmationCodeVersion) -> String {
        confirmation_code(&self.secret, version)
    }

    pub(crate) fn seal(&self, passphrase: &SecretBytes) -> Result<String> {
        seal(&self.secret, passphrase)
    }

    pub(crate) fn open(&self, sealed: &str) -> Result<SecretBytes> {
        let key = secret_key(&self.secret)?;
        let sealed =
            rikka_proton_api::base64::decode(sealed).map_err(|_| Error::DeviceSecretDecryption)?;
        let decrypted = key
            .open(sealed, SECRET_CONTEXT)
            .map_err(|_| Error::DeviceSecretDecryption)?;

        Ok(SecretBytes::new(decrypted))
    }
}

impl fmt::Debug for DeviceSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceSecret")
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

pub(crate) fn generate_secret() -> Secret {
    Secret::new(rikka_proton_api::base64::encode(
        random_bytes::<SECRET_LEN>(),
    ))
}

pub(crate) fn seal(secret: &Secret, passphrase: &SecretBytes) -> Result<String> {
    let sealed = secret_key(secret)?.seal(passphrase.expose(), SECRET_CONTEXT)?;

    Ok(rikka_proton_api::base64::encode(sealed))
}

pub(crate) fn confirmation_code(secret: &Secret, version: ConfirmationCodeVersion) -> String {
    let digest = sha256(secret.expose().as_bytes());
    let hashed = match version {
        ConfirmationCodeVersion::HexDigest => rikka_types::hex::encode(digest).into_bytes(),
        ConfirmationCodeVersion::Digest => digest.to_vec(),
    };

    let mut code = base32::encode_with(hashed, Alphabet::Crockford);
    code.truncate(CONFIRMATION_CODE_LEN);
    code
}

pub(crate) fn codes_match(expected: &str, entered: &str) -> bool {
    let normalized: String = entered
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '-')
        .map(|character| match character.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            other => other,
        })
        .collect();

    normalized == expected
}

fn secret_key(secret: &Secret) -> Result<SymmetricKey> {
    let bytes = rikka_proton_api::base64::decode(secret.expose())
        .ok()
        .filter(|bytes| bytes.len() == SECRET_LEN)
        .ok_or(Error::MalformedDeviceSecret)?;
    let bytes = SecretBytes::new(bytes);

    SymmetricKey::from_bytes(bytes.expose()).map_err(|_| Error::MalformedDeviceSecret)
}
