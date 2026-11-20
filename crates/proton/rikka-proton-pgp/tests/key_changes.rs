use proton_rpgp::DEFAULT_PROFILE;
use proton_rpgp::pgp::composed::{
    Deserializable as _, EncryptionCaps, KeyType, SecretKeyParamsBuilder, SignedPublicKey,
    SubkeyParamsBuilder,
};
use proton_rpgp::pgp::crypto::ecc_curve::ECCCurve;
use proton_rpgp::pgp::packet::SignatureType;
use proton_rpgp::pgp::ser::Serialize as _;
use proton_rpgp::pgp::types::{KeyDetails as _, Timestamp};
use rikka_proton_pgp::{
    Clock, Error, LockedKey, Pgp, PrivateKey, RevocationReason, SignatureStatus, is_armored,
};

const CREATED: u64 = 1_700_000_000;
const EXPIRES: u64 = CREATED + 1_000;
const SUBKEYS_CREATED: u64 = CREATED + 5;
const PASSPHRASE: &[u8] = b"changing keys";

fn pgp() -> Pgp {
    Pgp::with_clock(Clock::fixed(CREATED))
}

fn key() -> PrivateKey {
    pgp()
        .generate_key()
        .user_id("Rikka Changes", "changes@proton.me")
        .generate()
        .unwrap()
}

fn key_with_signing_subkey() -> PrivateKey {
    let subkeys_created = Timestamp::from_secs(SUBKEYS_CREATED as u32);
    let signing = SubkeyParamsBuilder::default()
        .key_type(KeyType::Ed25519Legacy)
        .can_sign(true)
        .created_at(subkeys_created)
        .build()
        .unwrap();
    let encryption = SubkeyParamsBuilder::default()
        .key_type(KeyType::ECDH(ECCCurve::Curve25519))
        .can_encrypt(EncryptionCaps::All)
        .created_at(subkeys_created)
        .build()
        .unwrap();
    let generated = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Ed25519Legacy)
        .can_certify(true)
        .can_sign(true)
        .created_at(Timestamp::from_secs(CREATED as u32))
        .primary_user_id("Rikka Subkeys <subkeys@proton.me>".to_owned())
        .subkeys(vec![signing, encryption])
        .build()
        .unwrap()
        .generate(DEFAULT_PROFILE.rng())
        .unwrap();
    PrivateKey::import_unlocked(generated.to_bytes().unwrap()).unwrap()
}

#[test]
fn expiration_marks_signatures_expired() {
    let pgp = pgp();
    let key = key();
    assert_eq!(key.expires_at().unwrap(), None);

    let expiring = pgp
        .change_expiration(&key)
        .expires_at(EXPIRES)
        .with_time(CREATED + 10)
        .apply()
        .unwrap();
    assert_eq!(expiring.fingerprint(), key.fingerprint());
    assert_eq!(expiring.expires_at().unwrap(), Some(EXPIRES));
    assert!(!expiring.is_expired_at(EXPIRES - 1));
    assert!(expiring.is_expired_at(EXPIRES + 1));

    let reimported = LockedKey::import(expiring.lock(PASSPHRASE).unwrap().export().unwrap())
        .unwrap()
        .unlock(PASSPHRASE)
        .unwrap();
    assert_eq!(reimported.expires_at().unwrap(), Some(EXPIRES));

    let signature = pgp
        .sign()
        .with_key(expiring.clone())
        .with_time(CREATED + 20)
        .detached(b"signed while valid")
        .unwrap();
    let verified_at = |time: u64, key: &PrivateKey| {
        pgp.verify()
            .with_key(key.public_key())
            .with_time(time)
            .detached(b"signed while valid", &signature)
    };

    assert!(verified_at(CREATED + 500, &expiring).is_valid());
    assert!(verified_at(EXPIRES + 5_000, &expiring).is_valid());

    let lapsed = pgp
        .change_expiration(&key)
        .expires_at(CREATED + 15)
        .with_time(CREATED + 12)
        .apply()
        .unwrap();
    assert_eq!(
        verified_at(CREATED + 500, &lapsed),
        SignatureStatus::Expired {
            key_id: lapsed.key_id(),
            created_at: CREATED + 20,
        }
    );

    let renewed = pgp
        .change_expiration(&lapsed)
        .never_expires()
        .with_time(CREATED + 30)
        .apply()
        .unwrap();
    assert_eq!(renewed.expires_at().unwrap(), None);
    assert!(verified_at(CREATED + 500, &renewed).is_valid());

    let refused = pgp.change_expiration(&key).expires_at(CREATED).apply();
    assert!(matches!(refused, Err(Error::Invalid(_))), "{refused:?}");
}

#[test]
fn revocation_certificate_revokes_key() {
    let pgp = pgp();
    let stranger = key();
    let key = key();
    let after = CREATED + 100;
    assert!(!key.public_key().is_revoked_at(after));

    let certificate = pgp
        .revoke(&key)
        .reason(RevocationReason::Compromised)
        .description("the laptop was stolen")
        .with_time(CREATED + 10)
        .certificate()
        .unwrap();
    let armored = pgp
        .revoke(&key)
        .reason(RevocationReason::Superseded)
        .with_time(CREATED + 10)
        .armored_certificate()
        .unwrap();
    assert!(is_armored(&armored));

    for certificate in [certificate.as_slice(), armored.as_bytes()] {
        let revoked = key
            .public_key()
            .with_revocation_certificate(certificate)
            .unwrap();
        assert!(revoked.is_revoked_at(after));
        assert!(!revoked.can_encrypt_at(after));
        assert!(
            pgp.encrypt()
                .to(revoked)
                .with_time(after)
                .message(b"to a revoked key")
                .is_err()
        );
    }

    let foreign = stranger
        .public_key()
        .with_revocation_certificate(&certificate);
    assert!(foreign.is_err(), "{foreign:?}");

    let revoked = pgp
        .revoke(&key)
        .reason(RevocationReason::Retired)
        .with_time(CREATED + 10)
        .apply()
        .unwrap();
    assert_eq!(revoked.fingerprint(), key.fingerprint());
    assert!(revoked.public_key().is_revoked_at(after));
}

#[test]
fn expiration_rebinds_subkeys() {
    let pgp = pgp();
    let key = key_with_signing_subkey();

    let changed = pgp
        .change_expiration(&key)
        .expires_at(EXPIRES)
        .with_time(CREATED + 10)
        .apply()
        .unwrap();
    let reimported = LockedKey::import(changed.lock(PASSPHRASE).unwrap().export().unwrap())
        .unwrap()
        .unlock(PASSPHRASE)
        .unwrap();
    assert_eq!(reimported.expires_at().unwrap(), Some(EXPIRES));

    let exported = reimported.public_key().export().unwrap();
    let public = SignedPublicKey::from_bytes(&exported[..]).unwrap();
    public.verify_bindings().unwrap();
    assert_eq!(public.public_subkeys.len(), 2);

    let mut back_signed = 0;
    for subkey in &public.public_subkeys {
        let bindings: Vec<_> = subkey
            .signatures
            .iter()
            .filter(|signature| signature.typ() == Some(SignatureType::SubkeyBinding))
            .collect();
        assert_eq!(bindings.len(), 1, "old binding not replaced");
        let binding = bindings[0];
        assert_eq!(
            binding
                .key_expiration_time()
                .map(|lifetime| lifetime.as_secs()),
            Some((EXPIRES - SUBKEYS_CREATED) as u32)
        );
        assert_eq!(
            u64::from(subkey.key.created_at().as_secs()),
            SUBKEYS_CREATED
        );

        if binding.key_flags().sign() {
            binding
                .embedded_signature()
                .unwrap()
                .verify_primary_key_binding(&subkey.key, &public.primary_key)
                .unwrap();
            back_signed += 1;
        }
    }
    assert_eq!(
        back_signed, 1,
        "only the signing subkey should be back-signed"
    );

    let signature = pgp
        .sign()
        .with_key(reimported.clone())
        .with_time(CREATED + 20)
        .detached(b"signed by a rebound subkey")
        .unwrap();
    assert!(
        pgp.verify()
            .with_key(reimported.public_key())
            .with_time(CREATED + 500)
            .detached(b"signed by a rebound subkey", &signature)
            .is_valid()
    );
}
