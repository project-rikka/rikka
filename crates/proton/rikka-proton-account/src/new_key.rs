use rikka_proton_pgp::{KeyKind, Pgp, PrivateKey, SecretBytes};

use crate::address_key_token::AddressKeyToken;
use crate::error::Result;

pub(crate) const USER_KEY_USER_ID: &str = "not_for_email_use@domain.tld";

const LEGACY_USER_ID: &str = "UserID";

pub(crate) struct NewUserKey {
    pub(crate) key: PrivateKey,
    pub(crate) armored: String,
}

pub(crate) struct NewAddressKey {
    pub(crate) key: PrivateKey,
    pub(crate) armored: String,
    pub(crate) token: Option<AddressKeyToken>,
}

pub(crate) enum AddressKeyLock<'a> {
    Token(&'a PrivateKey),
    Passphrase(&'a SecretBytes),
}

impl NewUserKey {
    pub(crate) fn generate(
        pgp: &Pgp,
        kind: KeyKind,
        passphrase: &SecretBytes,
    ) -> Result<NewUserKey> {
        let key = pgp
            .generate_key()
            .user_id(USER_KEY_USER_ID, USER_KEY_USER_ID)
            .kind(kind)
            .generate()?;
        let armored = key.lock(passphrase)?.export_armored()?;

        Ok(NewUserKey { key, armored })
    }
}

impl NewAddressKey {
    pub(crate) fn generate(
        pgp: &Pgp,
        email: &str,
        kind: KeyKind,
        lock: AddressKeyLock<'_>,
    ) -> Result<NewAddressKey> {
        let key = pgp
            .generate_key()
            .user_id(email, email)
            .kind(kind)
            .generate()?;

        NewAddressKey::from_key(pgp, key, lock)
    }

    pub(crate) fn from_key(
        pgp: &Pgp,
        key: PrivateKey,
        lock: AddressKeyLock<'_>,
    ) -> Result<NewAddressKey> {
        let (token, armored) = match lock {
            AddressKeyLock::Token(user_key) => {
                let token = AddressKeyToken::generate(pgp, user_key)?;
                let armored = key.lock(token.secret.expose())?.export_armored()?;
                (Some(token), armored)
            }
            AddressKeyLock::Passphrase(passphrase) => {
                (None, key.lock(passphrase)?.export_armored()?)
            }
        };

        Ok(NewAddressKey {
            key,
            armored,
            token,
        })
    }
}

pub(crate) fn key_version(kind: KeyKind) -> u8 {
    match kind {
        KeyKind::Curve25519 => 4,
        KeyKind::PostQuantum => 6,
    }
}

pub(crate) fn replace_legacy_user_id(key: PrivateKey, email: &str) -> Result<PrivateKey> {
    let user_ids = key.public_key().user_ids();
    let is_legacy =
        !user_ids.is_empty() && user_ids.iter().all(|user_id| user_id == LEGACY_USER_ID);

    match is_legacy {
        true => Ok(key.with_user_id(email, email)?),
        false => Ok(key),
    }
}
