use jiff::Timestamp;
use rikka_proton_pgp::{Modulus, Pgp, PublicKey, Secret, SessionKey, Srp, random_bytes};
use serde::Serialize;

use crate::crypto;
use crate::error::Result;
use crate::role::Role;
use crate::sharing::share_url::ShareUrlFlags;

pub(super) const GENERATED_LENGTH: usize = 12;
const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const SALT_BYTES: usize = 16;

#[derive(Debug, Serialize)]
pub(super) struct UrlCrypto {
    #[serde(rename = "Permissions")]
    pub(super) role: Role,
    #[serde(rename = "Flags")]
    pub(super) flags: ShareUrlFlags,
    #[serde(
        rename = "ExpirationTime",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    pub(super) expiration_time: Option<Timestamp>,
    #[serde(rename = "SharePasswordSalt")]
    pub(super) share_password_salt: String,
    #[serde(rename = "SharePassphraseKeyPacket")]
    pub(super) share_passphrase_key_packet: String,
    #[serde(rename = "Password")]
    pub(super) password: String,
    #[serde(rename = "UrlPasswordSalt")]
    pub(super) url_password_salt: String,
    #[serde(rename = "SRPVerifier")]
    pub(super) srp_verifier: String,
    #[serde(rename = "SRPModulusID")]
    pub(super) srp_modulus_id: String,
    #[serde(rename = "MaxAccesses")]
    pub(super) max_accesses: u64,
}

pub(super) struct Sealing<'a> {
    pub(super) pgp: Pgp,
    pub(super) modulus: Modulus,
    pub(super) address: PublicKey,
    pub(super) session_key: &'a SessionKey,
    pub(super) generated: &'a Secret,
    pub(super) custom: Option<&'a Secret>,
    pub(super) role: Role,
    pub(super) expiration_time: Option<Timestamp>,
}

pub(super) fn generate_password() -> Secret {
    let mut password = String::with_capacity(GENERATED_LENGTH);
    let rejection_limit = 256 - 256 % CHARSET.len();
    while password.len() < GENERATED_LENGTH {
        for byte in random_bytes::<{ GENERATED_LENGTH * 2 }>() {
            if usize::from(byte) < rejection_limit && password.len() < GENERATED_LENGTH {
                password.push(char::from(CHARSET[usize::from(byte) % CHARSET.len()]));
            }
        }
    }
    Secret::new(password)
}

pub(crate) fn split(password: &str, flags: ShareUrlFlags) -> (Secret, Option<Secret>) {
    match flags {
        ShareUrlFlags::GeneratedPassword | ShareUrlFlags::GeneratedAndCustomPassword
            if password.len() > GENERATED_LENGTH && password.is_char_boundary(GENERATED_LENGTH) =>
        {
            let (generated, custom) = password.split_at(GENERATED_LENGTH);
            (Secret::new(generated), Some(Secret::new(custom)))
        }
        _ => (Secret::new(password), None),
    }
}

pub(super) fn sealed(sealing: Sealing<'_>) -> Result<UrlCrypto> {
    let password = match sealing.custom {
        Some(custom) => format!("{}{}", sealing.generated.expose(), custom.expose()),
        None => sealing.generated.expose().to_owned(),
    };
    let password = Secret::new(password);
    let salt = random_bytes::<SALT_BYTES>();
    let key_password = Srp::new().mailbox_passphrase(password.expose(), salt)?;
    let key_packet =
        crypto::password_key_packet(&sealing.pgp, sealing.session_key, key_password.as_bytes())?;
    let armored_password =
        crypto::encrypt_text(&sealing.pgp, &sealing.address, None, password.expose())?;
    let modulus_id = sealing.modulus.id().unwrap_or_default().to_owned();
    let verifier = Srp::new()
        .register()
        .password(password.expose())
        .modulus(sealing.modulus)
        .verifier()?;

    Ok(UrlCrypto {
        role: sealing.role,
        flags: match sealing.custom {
            Some(_) => ShareUrlFlags::GeneratedAndCustomPassword,
            None => ShareUrlFlags::GeneratedPassword,
        },
        expiration_time: sealing.expiration_time,
        share_password_salt: rikka_proton_api::base64::encode(salt),
        share_passphrase_key_packet: rikka_proton_api::base64::encode(key_packet),
        password: armored_password,
        url_password_salt: verifier.salt().to_owned(),
        srp_verifier: verifier.verifier().to_owned(),
        srp_modulus_id: modulus_id,
        max_accesses: 0,
    })
}
