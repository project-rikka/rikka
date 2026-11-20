use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, SecretBytes};

use crate::address_key::AddressKey;
use crate::error::{Error, Result};
use crate::key_token::KeyToken;
use crate::user_key::UserKey;

pub(crate) fn user_key(key: &UserKey, passphrase: &SecretBytes) -> Result<PrivateKey> {
    Ok(key.to_locked_key()?.unlock(passphrase)?)
}

pub(crate) fn address_key(
    pgp: &Pgp,
    key: &AddressKey,
    user_keys: &KeyRing,
    passphrase: Option<&SecretBytes>,
) -> Result<PrivateKey> {
    let secret = address_key_passphrase(pgp, key, user_keys, passphrase)?;

    Ok(key.to_locked_key()?.unlock(&secret)?)
}

pub(crate) fn address_key_passphrase(
    pgp: &Pgp,
    key: &AddressKey,
    user_keys: &KeyRing,
    passphrase: Option<&SecretBytes>,
) -> Result<SecretBytes> {
    if let Some(activation) = key.activation() {
        return activation_passphrase(pgp, key, activation, user_keys);
    }
    if key.is_migrated() {
        return token_passphrase(pgp, key, user_keys);
    }

    passphrase
        .cloned()
        .ok_or_else(|| Error::NoKeyToken(key.id().clone()))
}

fn activation_passphrase(
    pgp: &Pgp,
    key: &AddressKey,
    activation: &str,
    user_keys: &KeyRing,
) -> Result<SecretBytes> {
    let decrypted = pgp
        .decrypt()
        .with_ring(user_keys)
        .verified_by_ring(user_keys)
        .message(activation)?;

    if !decrypted.signature().is_valid() {
        return Err(Error::UnsignedActivation {
            key: key.id().clone(),
            status: Box::new(decrypted.signature().clone()),
        });
    }

    Ok(SecretBytes::new(decrypted.into_data()))
}

fn token_passphrase(pgp: &Pgp, key: &AddressKey, user_keys: &KeyRing) -> Result<SecretBytes> {
    let (Some(token), Some(signature)) = (key.token(), key.signature()) else {
        return Err(Error::NoKeyToken(key.id().clone()));
    };

    KeyToken::new(token, signature).open(pgp, user_keys)
}
