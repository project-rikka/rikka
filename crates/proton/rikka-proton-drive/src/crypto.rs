use rikka_proton_account::AccountKeys;
use rikka_proton_pgp::{
    LockedKey, Pgp, PrivateKey, PublicKey, Secret, SessionKey, SignatureContext, SignatureStatus,
    VerificationContext, unarmor,
};

use crate::attributes::ExtendedAttributes;
use crate::error::{Error, Result};
use crate::hash_key::HashKey;

const KEY_NAME: &str = "Drive key";
const PASSPHRASE_BYTES: usize = 32;

pub(crate) struct DecryptedKey {
    pub(crate) key: PrivateKey,
    pub(crate) passphrase: Secret,
    pub(crate) session_key: SessionKey,
    pub(crate) signature: SignatureStatus,
}

pub(crate) struct DecryptedName {
    pub(crate) name: String,
    pub(crate) signature: SignatureStatus,
    pub(crate) session_key: SessionKey,
}

pub(crate) struct NewKey {
    pub(crate) key: PrivateKey,
    pub(crate) armored_key: String,
    pub(crate) armored_passphrase: String,
    pub(crate) armored_signature: String,
}

pub(crate) struct SealedBlock {
    pub(crate) data: Vec<u8>,
    pub(crate) armored_signature: String,
    pub(crate) hash: [u8; 32],
}

pub(crate) struct SealedThumbnail {
    pub(crate) data: Vec<u8>,
    pub(crate) hash: [u8; 32],
}

pub(crate) fn decrypt_key(
    pgp: &Pgp,
    armored_key: &str,
    armored_passphrase: &str,
    armored_signature: Option<&str>,
    decryption: &[PrivateKey],
    verification: &[PublicKey],
) -> Result<DecryptedKey> {
    let packets = unarmor(armored_passphrase)?;
    let session_key = decrypt_session_key(pgp, decryption, &packets)?;

    decrypt_key_with_session_key(
        pgp,
        armored_key,
        armored_passphrase,
        session_key,
        armored_signature,
        verification,
    )
}

pub(crate) fn generate_key(pgp: &Pgp, parent: &PublicKey, signer: &PrivateKey) -> Result<NewKey> {
    generate_key_for(pgp, std::slice::from_ref(parent), signer)
}

pub(crate) fn generate_key_for(
    pgp: &Pgp,
    parents: &[PublicKey],
    signer: &PrivateKey,
) -> Result<NewKey> {
    let passphrase = generate_passphrase();
    let key = pgp.generate_key().user_id(KEY_NAME, "").generate()?;
    let armored_key = key.lock(passphrase.expose().as_bytes())?.export_armored()?;

    let session_key = pgp
        .encrypt()
        .to_keys(parents.iter().cloned())
        .session_key()?;
    let armored_passphrase = pgp
        .encrypt()
        .to_keys(parents.iter().cloned())
        .with_session_key(session_key.clone())
        .armored_message(passphrase.expose().as_bytes())?;
    let armored_signature = pgp
        .sign()
        .with_key(signer.clone())
        .armored_detached(passphrase.expose().as_bytes())?;

    Ok(NewKey {
        key,
        armored_key,
        armored_passphrase,
        armored_signature,
    })
}

pub(crate) fn key_packet(pgp: &Pgp, session_key: &SessionKey, to: &PublicKey) -> Result<Vec<u8>> {
    Ok(pgp.encrypt().to(to.clone()).key_packets(session_key)?)
}

pub(crate) fn password_key_packet(
    pgp: &Pgp,
    session_key: &SessionKey,
    password: &[u8],
) -> Result<Vec<u8>> {
    Ok(pgp
        .encrypt()
        .with_password(password)
        .key_packets(session_key)?)
}

pub(crate) fn decrypt_session_key(
    pgp: &Pgp,
    keys: &[PrivateKey],
    packets: &[u8],
) -> Result<SessionKey> {
    Ok(pgp
        .decrypt()
        .with_keys(keys.iter().cloned())
        .session_key(packets)?)
}

pub(crate) fn context_signature(
    pgp: &Pgp,
    signer: &PrivateKey,
    data: &[u8],
    context: &str,
) -> Result<Vec<u8>> {
    Ok(pgp
        .sign()
        .with_key(signer.clone())
        .with_context(SignatureContext::new(context).critical(true))
        .detached(data)?)
}

pub(crate) fn verify_context(
    pgp: &Pgp,
    verification: &[PublicKey],
    data: &[u8],
    signature: &[u8],
    context: &str,
) -> SignatureStatus {
    pgp.verify()
        .with_keys(verification.iter().cloned())
        .with_context(VerificationContext::new(context))
        .detached(data, signature)
}

pub(crate) fn encrypt_text(
    pgp: &Pgp,
    to: &PublicKey,
    signer: Option<&PrivateKey>,
    text: &str,
) -> Result<String> {
    let mut encryption = pgp.encrypt().to(to.clone());
    if let Some(signer) = signer {
        encryption = encryption.signed_by(signer.clone());
    }

    Ok(encryption.armored_message(text.as_bytes())?)
}

pub(crate) fn decrypt_text(
    pgp: &Pgp,
    keys: &[PrivateKey],
    armored: &str,
    field: &'static str,
) -> Result<String> {
    let plaintext = pgp
        .decrypt()
        .with_keys(keys.iter().cloned())
        .message(armored)?;

    String::from_utf8(plaintext.into_data()).map_err(|_| Error::NotText(field))
}

pub(crate) fn decrypt_key_with_session_key(
    pgp: &Pgp,
    armored_key: &str,
    armored_passphrase: &str,
    session_key: SessionKey,
    armored_signature: Option<&str>,
    verification: &[PublicKey],
) -> Result<DecryptedKey> {
    let plaintext = pgp
        .decrypt()
        .with_session_key(session_key.clone())
        .message(armored_passphrase)?;
    let passphrase = read_passphrase(plaintext.data())?;
    let signature = match armored_signature {
        Some(armored) => pgp
            .verify()
            .with_keys(verification.iter().cloned())
            .detached(plaintext.data(), armored),
        None => SignatureStatus::Missing,
    };
    let key = LockedKey::import(armored_key)?.unlock(passphrase.expose().as_bytes())?;

    Ok(DecryptedKey {
        key,
        passphrase,
        session_key,
        signature,
    })
}

pub(crate) fn decrypt_key_with_password(
    pgp: &Pgp,
    armored_key: &str,
    armored_passphrase: &str,
    password: &[u8],
) -> Result<DecryptedKey> {
    let packets = unarmor(armored_passphrase)?;
    let session_key = pgp
        .decrypt()
        .with_password(password)
        .session_key(&packets)?;

    decrypt_key_with_session_key(pgp, armored_key, armored_passphrase, session_key, None, &[])
}

pub(crate) fn encrypt_passphrase(
    pgp: &Pgp,
    parent: &PublicKey,
    session_key: &SessionKey,
    signer: &PrivateKey,
    passphrase: &Secret,
) -> Result<(String, String)> {
    let armored_passphrase = pgp
        .encrypt()
        .to(parent.clone())
        .with_session_key(session_key.clone())
        .armored_message(passphrase.expose().as_bytes())?;
    let armored_signature = pgp
        .sign()
        .with_key(signer.clone())
        .armored_detached(passphrase.expose().as_bytes())?;

    Ok((armored_passphrase, armored_signature))
}

pub(crate) fn encrypt_name(
    pgp: &Pgp,
    parent: &PublicKey,
    session_key: Option<&SessionKey>,
    signer: &PrivateKey,
    name: &str,
) -> Result<String> {
    let mut encryption = pgp.encrypt().to(parent.clone()).signed_by(signer.clone());
    if let Some(session_key) = session_key {
        encryption = encryption.with_session_key(session_key.clone());
    }

    Ok(encryption.armored_message(name.as_bytes())?)
}

pub(crate) fn decrypt_name(
    pgp: &Pgp,
    parent: &PrivateKey,
    verification: &[PublicKey],
    armored: &str,
) -> Result<DecryptedName> {
    let packets = unarmor(armored)?;
    let session_key = pgp
        .decrypt()
        .with_key(parent.clone())
        .session_key(&packets)?;
    let plaintext = pgp
        .decrypt()
        .with_session_key(session_key.clone())
        .verified_by_keys(verification.iter().cloned())
        .message(armored)?;
    let name =
        String::from_utf8(plaintext.data().to_vec()).map_err(|_| Error::NotText("node name"))?;

    Ok(DecryptedName {
        name,
        signature: plaintext.signature().clone(),
        session_key,
    })
}

pub(crate) fn generate_hash_key(pgp: &Pgp, key: &PrivateKey) -> Result<(HashKey, String)> {
    let secret = generate_passphrase();
    let armored = pgp
        .encrypt()
        .to(key.public_key())
        .signed_by(key.clone())
        .armored_message(secret.expose().as_bytes())?;

    Ok((HashKey::new(secret.expose().as_bytes().to_vec()), armored))
}

pub(crate) fn decrypt_hash_key(
    pgp: &Pgp,
    key: &PrivateKey,
    verification: &[PublicKey],
    armored: &str,
) -> Result<(HashKey, SignatureStatus)> {
    let plaintext = pgp
        .decrypt()
        .with_key(key.clone())
        .verified_by(key.public_key())
        .verified_by_keys(verification.iter().cloned())
        .message(armored)?;

    Ok((
        HashKey::new(plaintext.data().to_vec()),
        plaintext.signature().clone(),
    ))
}

pub(crate) fn generate_content_key(
    pgp: &Pgp,
    key: &PrivateKey,
) -> Result<(SessionKey, String, String)> {
    let session_key = pgp.encrypt().to(key.public_key()).session_key()?;
    let packet = pgp
        .encrypt()
        .to(key.public_key())
        .key_packets(&session_key)?;
    let armored_signature = pgp
        .sign()
        .with_key(key.clone())
        .armored_detached(session_key.as_bytes())?;

    Ok((
        session_key,
        rikka_proton_api::base64::encode(packet),
        armored_signature,
    ))
}

pub(crate) fn decrypt_content_key(
    pgp: &Pgp,
    key: &PrivateKey,
    verification: &[PublicKey],
    packet: &str,
    armored_signature: Option<&str>,
) -> Result<(SessionKey, SignatureStatus)> {
    let packet = rikka_proton_api::base64::decode(packet)
        .map_err(|_| Error::NotBase64("content key packet"))?;
    let session_key = pgp.decrypt().with_key(key.clone()).session_key(&packet)?;

    let signature = match armored_signature {
        Some(armored) => pgp
            .verify()
            .with_key(key.public_key())
            .with_keys(verification.iter().cloned())
            .detached(session_key.as_bytes(), armored),
        None => SignatureStatus::Missing,
    };

    Ok((session_key, signature))
}

pub(crate) fn encrypt_attributes(
    pgp: &Pgp,
    key: &PrivateKey,
    signer: &PrivateKey,
    attributes: &ExtendedAttributes,
) -> Result<String> {
    Ok(pgp
        .encrypt()
        .to(key.public_key())
        .signed_by(signer.clone())
        .armored_message(attributes.write()?.as_bytes())?)
}

pub(crate) fn decrypt_attributes(
    pgp: &Pgp,
    key: &PrivateKey,
    verification: &[PublicKey],
    armored: &str,
) -> Result<(ExtendedAttributes, SignatureStatus)> {
    let plaintext = pgp
        .decrypt()
        .with_key(key.clone())
        .verified_by(key.public_key())
        .verified_by_keys(verification.iter().cloned())
        .message(armored)?;

    Ok((
        ExtendedAttributes::read(plaintext.data())?,
        plaintext.signature().clone(),
    ))
}

pub(crate) fn seal_block(
    pgp: &Pgp,
    node: &PublicKey,
    content: &SessionKey,
    signer: &PrivateKey,
    plain: &[u8],
) -> Result<SealedBlock> {
    let data = pgp
        .encrypt()
        .with_session_key(content.clone())
        .message(plain)?;
    let signature = pgp.sign().with_key(signer.clone()).detached(plain)?;
    let armored_signature = pgp
        .encrypt()
        .to(node.clone())
        .with_session_key(content.clone())
        .armored_message(signature)?;

    Ok(SealedBlock {
        hash: digest(&data),
        data,
        armored_signature,
    })
}

pub(crate) fn seal_thumbnail(
    pgp: &Pgp,
    content: &SessionKey,
    signer: &PrivateKey,
    plain: &[u8],
) -> Result<SealedThumbnail> {
    let data = pgp
        .encrypt()
        .with_session_key(content.clone())
        .signed_by(signer.clone())
        .message(plain)?;

    Ok(SealedThumbnail {
        hash: digest(&data),
        data,
    })
}

pub(crate) fn decrypt_block(pgp: &Pgp, content: &SessionKey, data: &[u8]) -> Result<Vec<u8>> {
    Ok(pgp
        .decrypt()
        .with_session_key(content.clone())
        .message(data)?
        .into_data())
}

pub(crate) fn sign_manifest(pgp: &Pgp, signer: &PrivateKey, manifest: &[u8]) -> Result<String> {
    Ok(pgp
        .sign()
        .with_key(signer.clone())
        .armored_detached(manifest)?)
}

pub(crate) fn verify_manifest(
    pgp: &Pgp,
    verification: &[PublicKey],
    manifest: &[u8],
    armored: &str,
) -> SignatureStatus {
    pgp.verify()
        .with_keys(verification.iter().cloned())
        .detached(manifest, armored)
}

pub(crate) fn digest(data: &[u8]) -> [u8; 32] {
    rikka_proton_pgp::sha256(data)
}

fn generate_passphrase() -> Secret {
    Secret::random_base64(PASSPHRASE_BYTES)
}

fn read_passphrase(bytes: &[u8]) -> Result<Secret> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::NotText("passphrase"))?;
    Ok(Secret::new(text))
}

pub(crate) fn primary_address_key(
    keys: &AccountKeys,
    address: &rikka_proton_account::AddressId,
) -> Result<(rikka_proton_account::KeyId, PrivateKey)> {
    let no_key = || rikka_proton_account::Error::NoAddressKey(address.clone());
    let primary_key = keys
        .address(address)
        .and_then(rikka_proton_account::Address::primary_key)
        .ok_or_else(no_key)?;
    let key = keys
        .address_key(primary_key.id())
        .cloned()
        .ok_or_else(no_key)?;

    Ok((primary_key.id().clone(), key))
}

pub(crate) fn signer(
    keys: &AccountKeys,
    address: &rikka_proton_account::AddressId,
) -> Result<PrivateKey> {
    Ok(keys.signer(address)?.clone())
}

pub(crate) fn email(
    keys: &AccountKeys,
    address: &rikka_proton_account::AddressId,
) -> Result<String> {
    Ok(keys.known_address(address)?.email().to_owned())
}

pub(crate) fn verification_keys(keys: &AccountKeys, email: Option<&str>) -> Vec<PublicKey> {
    email
        .and_then(|email| keys.address_by_email(email))
        .map(|address| keys.verification_keys(address.id()))
        .unwrap_or_default()
}
