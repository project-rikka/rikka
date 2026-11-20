use rikka_proton_account::UserKeys;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, SignatureContext, VerificationContext};

use crate::api::InviteKey;
use crate::content::Vault;
use crate::crypto::key::EncryptionKey;
use crate::crypto::rotated::RotatedKey;
use crate::crypto::tag::EncryptionTag;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::{Error, Result};

const EXISTING_USER: &str = "pass.invite.vault.existing-user";
const NEW_USER: &str = "pass.invite.vault.new-user";

pub(crate) fn encrypt_for_invitee(
    pgp: &Pgp,
    inviter: &PrivateKey,
    invitee: &PublicKey,
    keys: &[RotatedKey],
) -> Result<Vec<InviteKey>> {
    keys.iter()
        .map(|rotated| {
            let encrypted = pgp
                .encrypt()
                .to(invitee.clone())
                .signed_by(inviter.clone())
                .with_context(SignatureContext::new(EXISTING_USER).critical(true))
                .message(rotated.key.as_bytes())?;

            Ok(InviteKey::new(rotated.rotation, encode_base64(encrypted)))
        })
        .collect()
}

pub(crate) fn decrypt_invite_key(
    pgp: &Pgp,
    invited: &UserKeys,
    inviter: &str,
    inviter_keys: &[PublicKey],
    key: &InviteKey,
) -> Result<EncryptionKey> {
    let message = decode_base64("invite key", key.key())?;
    let decrypted = pgp
        .decrypt()
        .with_ring(invited.ring())
        .verified_by_keys(inviter_keys.iter().cloned())
        .with_context(VerificationContext::new(EXISTING_USER).required(true))
        .message(message)?;

    if !decrypted.signature().is_valid() {
        return Err(Error::InvalidInviteSignature {
            inviter: inviter.to_owned(),
            status: decrypted.signature().clone(),
        });
    }

    EncryptionKey::from_bytes(decrypted.into_data())
}

pub(crate) fn reencrypt_for_account(
    pgp: &Pgp,
    invited: &UserKeys,
    account: &UserKeys,
    inviter: &str,
    inviter_keys: &[PublicKey],
    keys: &[InviteKey],
) -> Result<Vec<InviteKey>> {
    keys.iter()
        .map(|key| {
            let decrypted = decrypt_invite_key(pgp, invited, inviter, inviter_keys, key)?;

            Ok(InviteKey::new(
                key.rotation(),
                decrypted.wrap(pgp, account)?,
            ))
        })
        .collect()
}

pub(crate) fn decrypt_vault(key: &EncryptionKey, content: &str) -> Result<Vault> {
    let sealed = decode_base64("invite vault content", content)?;
    let decrypted = key.open(sealed, &EncryptionTag::VaultContent)?;

    Vault::decode(&decrypted)
}

pub(crate) fn new_user_signature(
    pgp: &Pgp,
    inviter: &PrivateKey,
    email: &str,
    key: &EncryptionKey,
) -> Result<String> {
    let signature = pgp
        .sign()
        .with_key(inviter.clone())
        .with_context(SignatureContext::new(NEW_USER).critical(true))
        .detached(signature_body(email, key))?;

    Ok(encode_base64(signature))
}

pub(crate) fn verify_new_user_signature(
    pgp: &Pgp,
    inviter: &UserKeys,
    email: &str,
    key: &EncryptionKey,
    signature: &str,
) -> Result<()> {
    let signature = decode_base64("new user invite signature", signature)?;
    let status = pgp
        .verify()
        .with_ring(inviter.ring())
        .with_context(VerificationContext::new(NEW_USER).required(true))
        .detached(signature_body(email, key), signature);

    if !status.is_valid() {
        return Err(Error::InvalidInviteSignature {
            inviter: email.to_owned(),
            status,
        });
    }
    Ok(())
}

fn signature_body(email: &str, key: &EncryptionKey) -> Vec<u8> {
    let mut body = Vec::with_capacity(email.len() + 1 + key.as_bytes().len());
    body.extend_from_slice(email.as_bytes());
    body.push(b'|');
    body.extend_from_slice(key.as_bytes());
    body
}
