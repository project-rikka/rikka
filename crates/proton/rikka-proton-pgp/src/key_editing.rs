use proton_rpgp::pgp::bytes::Bytes;
use proton_rpgp::pgp::composed::{
    Deserializable as _, DetachedSignature, SignedPublicKey, SignedSecretKey,
};
use proton_rpgp::pgp::packet::{
    PacketTrait as _, PublicKey as PrimaryPublicKey, RevocationCode, SecretKey, SecretSubkey,
    Signature, SignatureConfig, SignatureType, Subpacket, SubpacketData,
};
use proton_rpgp::pgp::ser::Serialize as _;
use proton_rpgp::pgp::types::{Duration, KeyDetails as _, KeyVersion, Password, Tag, Timestamp};
use proton_rpgp::{DEFAULT_PROFILE, DataEncoding};
use zeroize::Zeroizing;

use crate::armor::{is_armored, unarmor};
use crate::error::{Error, Result};

pub(crate) fn set_expiration(
    key: &proton_rpgp::PrivateKey,
    expires_at: Option<u64>,
    time: u64,
) -> Result<proton_rpgp::PrivateKey> {
    let mut secret = decode_secret_key(key)?;
    let primary = secret.primary_key.clone();
    let public = primary.public_key().clone();
    let signed_at = timestamp(time)?;
    let lifetime = key_lifetime(public.created_at(), expires_at)?;

    for user in &mut secret.details.users {
        let Some(latest) = latest_self_signature(&user.signatures, &public) else {
            continue;
        };
        let config = renewed_config(latest, &primary, signed_at, lifetime)?;
        let signature = config
            .sign_certification(&primary, &public, &Password::empty(), Tag::UserId, &user.id)
            .map_err(modification_error)?;
        user.signatures.retain(|certification| {
            !(certification.is_certification() && issued_by(certification, &public))
        });
        user.signatures.push(signature);
    }

    let direct = &mut secret.details.direct_signatures;
    if let Some(latest) = latest_self_signature(direct, &public) {
        let config = renewed_config(latest, &primary, signed_at, lifetime)?;
        let signature = config
            .sign_key(&primary, &Password::empty(), &public)
            .map_err(modification_error)?;
        direct.retain(|certification| {
            !(certification.typ() == Some(SignatureType::Key) && issued_by(certification, &public))
        });
        direct.push(signature);
    }

    for subkey in &mut secret.secret_subkeys {
        let Some(latest) = latest_subkey_binding(&subkey.signatures, &public) else {
            continue;
        };
        let lifetime = key_lifetime(subkey.key.created_at(), expires_at)?;
        let mut config = renewed_config(latest, &primary, signed_at, lifetime)?;
        if latest.key_flags().sign() {
            let back = back_signature(&subkey.key, &public, signed_at)?;
            config
                .hashed_subpackets
                .push(subpacket(SubpacketData::EmbeddedSignature(Box::new(back)))?);
        }
        let binding = config
            .sign_subkey_binding(
                &primary,
                &public,
                &Password::empty(),
                subkey.key.public_key(),
            )
            .map_err(modification_error)?;
        replace_subkey_binding(&mut subkey.signatures, binding, &public);
    }

    for subkey in &mut secret.public_subkeys {
        let Some(latest) = latest_subkey_binding(&subkey.signatures, &public) else {
            continue;
        };
        if latest.key_flags().sign() {
            return Err(Error::Invalid(
                "cannot rebind signing subkey without secret key material",
            ));
        }
        let lifetime = key_lifetime(subkey.key.created_at(), expires_at)?;
        let binding = renewed_config(latest, &primary, signed_at, lifetime)?
            .sign_subkey_binding(&primary, &public, &Password::empty(), &subkey.key)
            .map_err(modification_error)?;
        replace_subkey_binding(&mut subkey.signatures, binding, &public);
    }

    encode_secret_key(&secret)
}

pub(crate) fn revocation_signature(
    key: &proton_rpgp::PrivateKey,
    code: RevocationCode,
    description: &str,
    time: u64,
) -> Result<Signature> {
    let secret = decode_secret_key(key)?;
    let primary = &secret.primary_key;
    let public = primary.public_key();

    let mut config = signature_config(primary, SignatureType::KeyRevocation)?;
    config.hashed_subpackets = vec![
        subpacket(SubpacketData::SignatureCreationTime(timestamp(time)?))?,
        subpacket(SubpacketData::IssuerFingerprint(public.fingerprint()))?,
        subpacket(SubpacketData::RevocationReason(
            code,
            Bytes::copy_from_slice(description.as_bytes()),
        ))?,
    ];
    if public.version() == KeyVersion::V4 {
        config.unhashed_subpackets = vec![subpacket(SubpacketData::IssuerKeyId(
            public.legacy_key_id(),
        ))?];
    }

    config
        .sign_key(primary, &Password::empty(), public)
        .map_err(modification_error)
}

pub(crate) fn add_revocation(
    key: &proton_rpgp::PrivateKey,
    revocation: Signature,
) -> Result<proton_rpgp::PrivateKey> {
    let mut secret = decode_secret_key(key)?;
    secret.details.revocation_signatures.push(revocation);
    encode_secret_key(&secret)
}

pub(crate) fn encode_signature(signature: &Signature) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    signature
        .to_writer_with_header(&mut encoded)
        .map_err(modification_error)?;
    Ok(encoded)
}

pub(crate) fn with_revocation_certificate(
    key: &proton_rpgp::PublicKey,
    certificate: &[u8],
) -> Result<proton_rpgp::PublicKey> {
    let binary = match is_armored(certificate) {
        true => unarmor(certificate)?,
        false => certificate.to_vec(),
    };
    let revocation = DetachedSignature::from_bytes(&binary[..])
        .map_err(|error| Error::Malformed(Box::new(error)))?
        .signature;
    if revocation.typ() != Some(SignatureType::KeyRevocation) {
        return Err(Error::Invalid("signature is not a key revocation"));
    }

    let encoded = key.export(DataEncoding::Unarmored)?;
    let mut public = SignedPublicKey::from_bytes(&encoded[..])
        .map_err(|error| Error::Malformed(Box::new(error)))?;
    revocation
        .verify_key(&public.primary_key)
        .map_err(|error| Error::KeyModification(Box::new(error)))?;
    public.details.revocation_signatures.push(revocation);

    let encoded = public.to_bytes().map_err(modification_error)?;
    Ok(proton_rpgp::PublicKey::import(
        &encoded,
        DataEncoding::Unarmored,
    )?)
}

fn decode_secret_key(key: &proton_rpgp::PrivateKey) -> Result<SignedSecretKey> {
    let encoded = Zeroizing::new(key.export_unlocked(DataEncoding::Unarmored)?);
    SignedSecretKey::from_bytes(&encoded[..]).map_err(|error| Error::Malformed(Box::new(error)))
}

fn encode_secret_key(secret: &SignedSecretKey) -> Result<proton_rpgp::PrivateKey> {
    let encoded = Zeroizing::new(secret.to_bytes().map_err(modification_error)?);
    Ok(proton_rpgp::PrivateKey::import_unlocked(
        &encoded,
        DataEncoding::Unarmored,
    )?)
}

fn renewed_config(
    latest: &Signature,
    primary: &SecretKey,
    signed_at: Timestamp,
    lifetime: Option<Duration>,
) -> Result<SignatureConfig> {
    let previous = latest
        .config()
        .ok_or(Error::Invalid("unsupported self-signature version"))?;
    let mut config = match primary.version() {
        KeyVersion::V6 => SignatureConfig::v6(
            DEFAULT_PROFILE.rng(),
            previous.typ,
            previous.pub_alg,
            previous.hash_alg,
        )
        .map_err(modification_error)?,
        _ => SignatureConfig::v4(previous.typ, previous.pub_alg, previous.hash_alg),
    };

    let mut hashed = vec![subpacket(SubpacketData::SignatureCreationTime(signed_at))?];
    hashed.extend(
        previous
            .hashed_subpackets
            .iter()
            .filter(|subpacket| {
                !matches!(
                    subpacket.data,
                    SubpacketData::SignatureCreationTime(_)
                        | SubpacketData::SignatureExpirationTime(_)
                        | SubpacketData::KeyExpirationTime(_)
                        | SubpacketData::EmbeddedSignature(_)
                )
            })
            .cloned(),
    );
    if let Some(lifetime) = lifetime {
        hashed.push(subpacket(SubpacketData::KeyExpirationTime(lifetime))?);
    }

    config.hashed_subpackets = hashed;
    config.unhashed_subpackets = previous
        .unhashed_subpackets
        .iter()
        .filter(|subpacket| !matches!(subpacket.data, SubpacketData::EmbeddedSignature(_)))
        .cloned()
        .collect();
    Ok(config)
}

fn back_signature(
    subkey: &SecretSubkey,
    primary: &PrimaryPublicKey,
    signed_at: Timestamp,
) -> Result<Signature> {
    let mut config =
        SignatureConfig::from_key(DEFAULT_PROFILE.rng(), subkey, SignatureType::KeyBinding)
            .map_err(modification_error)?;
    config.hashed_subpackets = vec![
        subpacket(SubpacketData::SignatureCreationTime(signed_at))?,
        subpacket(SubpacketData::IssuerFingerprint(subkey.fingerprint()))?,
    ];
    if subkey.version() == KeyVersion::V4 {
        config.unhashed_subpackets = vec![subpacket(SubpacketData::IssuerKeyId(
            subkey.legacy_key_id(),
        ))?];
    }
    config
        .sign_primary_key_binding(subkey, subkey.public_key(), &Password::empty(), primary)
        .map_err(modification_error)
}

fn latest_subkey_binding<'a>(
    signatures: &'a [Signature],
    public: &PrimaryPublicKey,
) -> Option<&'a Signature> {
    signatures
        .iter()
        .filter(|signature| {
            signature.typ() == Some(SignatureType::SubkeyBinding) && issued_by(signature, public)
        })
        .max_by_key(|signature| signature.created().map(Timestamp::as_secs))
}

fn replace_subkey_binding(
    signatures: &mut Vec<Signature>,
    binding: Signature,
    public: &PrimaryPublicKey,
) {
    signatures.retain(|certification| {
        !(certification.typ() == Some(SignatureType::SubkeyBinding)
            && issued_by(certification, public))
    });
    signatures.push(binding);
}

fn signature_config(primary: &SecretKey, typ: SignatureType) -> Result<SignatureConfig> {
    let hash = DEFAULT_PROFILE.key_hash_algorithm();
    match primary.version() {
        KeyVersion::V6 => {
            SignatureConfig::v6(DEFAULT_PROFILE.rng(), typ, primary.algorithm(), hash)
                .map_err(modification_error)
        }
        _ => Ok(SignatureConfig::v4(typ, primary.algorithm(), hash)),
    }
}

fn latest_self_signature<'a>(
    signatures: &'a [Signature],
    public: &PrimaryPublicKey,
) -> Option<&'a Signature> {
    signatures
        .iter()
        .filter(|signature| {
            (signature.is_certification() || signature.typ() == Some(SignatureType::Key))
                && issued_by(signature, public)
        })
        .max_by_key(|signature| signature.created().map(Timestamp::as_secs))
}

fn issued_by(signature: &Signature, public: &PrimaryPublicKey) -> bool {
    let fingerprint = public.fingerprint();
    let key_id = public.legacy_key_id();
    signature
        .issuer_fingerprint()
        .into_iter()
        .any(|issuer| *issuer == fingerprint)
        || signature
            .issuer_key_id()
            .into_iter()
            .any(|issuer| *issuer == key_id)
}

fn key_lifetime(created: Timestamp, expires_at: Option<u64>) -> Result<Option<Duration>> {
    let Some(expires_at) = expires_at else {
        return Ok(None);
    };
    let created = u64::from(created.as_secs());
    let seconds = expires_at
        .checked_sub(created)
        .filter(|seconds| *seconds > 0)
        .ok_or(Error::Invalid("key expiration must follow creation time"))?;
    let seconds = u32::try_from(seconds)
        .map_err(|_| Error::Invalid("key lifetime exceeds 32-bit seconds"))?;
    Ok(Some(Duration::from_secs(seconds)))
}

fn timestamp(unix_seconds: u64) -> Result<Timestamp> {
    u32::try_from(unix_seconds)
        .map(Timestamp::from_secs)
        .map_err(|_| Error::Invalid("timestamp exceeds 32-bit OpenPGP range"))
}

fn subpacket(data: SubpacketData) -> Result<Subpacket> {
    Subpacket::regular(data).map_err(modification_error)
}

fn modification_error(error: proton_rpgp::pgp::errors::Error) -> Error {
    Error::KeyModification(Box::new(error))
}
