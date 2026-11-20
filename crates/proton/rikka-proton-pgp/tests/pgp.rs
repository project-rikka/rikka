use rikka_proton_pgp::{
    Clock, Error, KeyKind, LockedKey, Pgp, PrivateKey, PublicKey, SignatureContext,
    SignatureStatus, VerificationContext, armor_message, is_armored, unarmor,
};

const PASSPHRASE: &[u8] = b"correct horse battery staple";

fn pgp() -> Pgp {
    Pgp::with_clock(Clock::fixed(1_700_000_000))
}

fn key() -> PrivateKey {
    pgp()
        .generate_key()
        .user_id("Rikka Test", "test@proton.me")
        .generate()
        .unwrap()
}

#[test]
fn signed_message_round_trips() {
    let pgp = pgp();
    let sender = key();
    let recipient = key();

    let message = pgp
        .encrypt()
        .to(recipient.public_key())
        .signed_by(sender.clone())
        .message(b"hello proton")
        .unwrap();

    let plaintext = pgp
        .decrypt()
        .with_key(recipient)
        .verified_by(sender.public_key())
        .message(&message)
        .unwrap();

    assert!(plaintext.signature().is_valid());
    assert_eq!(plaintext.signature().created_at(), Some(1_700_000_000));
    assert_eq!(plaintext.into_verified().unwrap(), b"hello proton");
}

#[test]
fn armored_message_round_trips_binary() {
    let pgp = pgp();
    let key = key();

    let armored = pgp
        .encrypt()
        .to(key.public_key())
        .armored_message(b"armored body")
        .unwrap();
    assert!(is_armored(&armored));

    let binary = unarmor(&armored).unwrap();
    assert_eq!(
        armor_message(&binary).unwrap(),
        armored.replace("\r\n", "\n")
    );

    let plaintext = pgp.decrypt().with_key(key).message(&binary).unwrap();
    assert_eq!(plaintext.data(), b"armored body");
}

#[test]
fn split_separates_key_and_data_packets() {
    let pgp = pgp();
    let key = key();

    let split = pgp
        .encrypt()
        .to(key.public_key())
        .split(b"drive block")
        .unwrap();

    assert!(!split.key_packets().is_empty());
    assert!(!split.data_packet().is_empty());

    let session_key = pgp
        .decrypt()
        .with_key(key.clone())
        .session_key(split.key_packets())
        .unwrap();
    assert_eq!(session_key.as_bytes(), split.session_key().as_bytes());

    let plaintext = pgp
        .decrypt()
        .with_session_key(session_key)
        .message(split.data_packet())
        .unwrap();
    assert_eq!(plaintext.data(), b"drive block");
    assert!(plaintext.signature().is_missing());
}

#[test]
fn password_message_requires_password() {
    let pgp = pgp();

    let message = pgp
        .encrypt()
        .with_password("open sesame")
        .message(b"password body")
        .unwrap();

    let plaintext = pgp
        .decrypt()
        .with_password("open sesame")
        .message(&message)
        .unwrap();
    assert_eq!(plaintext.data(), b"password body");

    let error = pgp
        .decrypt()
        .with_password("wrong")
        .message(&message)
        .unwrap_err();
    assert!(matches!(error, Error::DecryptionFailed(_)));
}

#[test]
fn detached_inline_cleartext_signatures_verify() {
    let pgp = pgp();
    let key = key();

    let detached = pgp
        .sign()
        .with_key(key.clone())
        .detached(b"signed body")
        .unwrap();
    assert!(
        pgp.verify()
            .with_key(key.public_key())
            .detached(b"signed body", &detached)
            .is_valid()
    );
    assert!(
        !pgp.verify()
            .with_key(key.public_key())
            .detached(b"other body", &detached)
            .is_valid()
    );

    let armored = pgp
        .sign()
        .with_key(key.clone())
        .armored_detached(b"signed body")
        .unwrap();
    assert!(armored.starts_with("-----BEGIN PGP SIGNATURE-----"));

    let inline = pgp
        .sign()
        .with_key(key.clone())
        .inline(b"inline body")
        .unwrap();
    let verified = pgp
        .verify()
        .with_key(key.public_key())
        .inline(&inline)
        .unwrap();
    assert_eq!(verified.into_verified().unwrap(), b"inline body");

    let cleartext = pgp
        .sign()
        .with_key(key.clone())
        .cleartext("cleartext body\n")
        .unwrap();
    assert!(cleartext.starts_with("-----BEGIN PGP SIGNED MESSAGE-----"));
    let verified = pgp
        .verify()
        .with_key(key.public_key())
        .cleartext(&cleartext)
        .unwrap();
    assert_eq!(verified.data(), b"cleartext body\n");
    assert!(verified.signature().is_valid());
}

#[test]
fn signature_context_must_match() {
    let pgp = pgp();
    let key = key();

    let signature = pgp
        .sign()
        .with_key(key.clone())
        .with_context(SignatureContext::new("account.key.proton.me").critical(true))
        .detached(b"contextual body")
        .unwrap();

    let status = pgp
        .verify()
        .with_key(key.public_key())
        .with_context(VerificationContext::new("account.key.proton.me"))
        .detached(b"contextual body", &signature);
    assert!(status.is_valid());

    let status = pgp
        .verify()
        .with_key(key.public_key())
        .with_context(VerificationContext::new("drive.share.proton.me"))
        .detached(b"contextual body", &signature);
    assert!(matches!(status, SignatureStatus::BadContext { .. }));

    let plain = pgp
        .sign()
        .with_key(key.clone())
        .detached(b"contextual body")
        .unwrap();
    let status = pgp
        .verify()
        .with_key(key.public_key())
        .with_context(VerificationContext::new("account.key.proton.me"))
        .detached(b"contextual body", &plain);
    assert!(matches!(status, SignatureStatus::BadContext { .. }));

    let status = pgp
        .verify()
        .with_key(key.public_key())
        .with_context(VerificationContext::new("account.key.proton.me").required(false))
        .detached(b"contextual body", &plain);
    assert!(status.is_valid());
}

#[test]
fn unknown_key_and_missing_signature_differ() {
    let pgp = pgp();
    let signer = key();
    let stranger = key();

    let signature = pgp
        .sign()
        .with_key(signer)
        .detached(b"unknown body")
        .unwrap();

    let status = pgp
        .verify()
        .with_key(stranger.public_key())
        .detached(b"unknown body", &signature);
    assert!(matches!(status, SignatureStatus::UnknownKey { .. }));

    let message = pgp
        .encrypt()
        .to(stranger.public_key())
        .message(b"unsigned body")
        .unwrap();
    let plaintext = pgp.decrypt().with_key(stranger).message(&message).unwrap();
    assert_eq!(*plaintext.signature(), SignatureStatus::Missing);

    let error = plaintext.into_verified().unwrap_err();
    assert!(matches!(
        error,
        Error::SignatureInvalid(SignatureStatus::Missing)
    ));
}

#[test]
fn locked_keys_round_trip() {
    let pgp = pgp();
    let key = key();

    let armored_public = key.public_key().export_armored().unwrap();
    let imported = PublicKey::import(&armored_public).unwrap();
    assert_eq!(imported.fingerprint(), key.fingerprint());

    let binary_public = key.public_key().export().unwrap();
    assert!(!is_armored(&binary_public));
    assert_eq!(
        PublicKey::import(&binary_public).unwrap().fingerprint(),
        key.fingerprint()
    );

    let locked = key.lock(PASSPHRASE).unwrap();
    assert!(locked.is_locked());
    let armored_private = locked.export_armored().unwrap();
    let locked = LockedKey::import(&armored_private).unwrap();
    assert!(locked.is_locked());
    assert_eq!(locked.fingerprint(), key.fingerprint());

    let error = locked.unlock(b"wrong passphrase").unwrap_err();
    assert!(matches!(error, Error::WrongPassphrase));

    let unlocked = locked.unlock(PASSPHRASE).unwrap();
    assert_eq!(unlocked.fingerprint(), key.fingerprint());

    let message = pgp
        .encrypt()
        .to(locked.public_key())
        .message(b"locked body")
        .unwrap();
    let plaintext = pgp.decrypt().with_key(unlocked).message(&message).unwrap();
    assert_eq!(plaintext.data(), b"locked body");
}

#[test]
fn key_ring_encrypts_to_all_keys() {
    let pgp = pgp();
    let first = key();
    let second = key();
    let ring = rikka_proton_pgp::KeyRing::new()
        .with_private(first.clone())
        .with_public(second.public_key());

    assert_eq!(ring.len(), 2);
    assert!(ring.find_private(&first.fingerprint()).is_some());
    assert!(
        ring.find_public(&second.public_key().fingerprint())
            .is_some()
    );

    let message = pgp
        .encrypt()
        .to_ring(&ring)
        .signed_by(first.clone())
        .message(b"ring body")
        .unwrap();

    let plaintext = pgp
        .decrypt()
        .with_ring(&ring)
        .verified_by_ring(&ring)
        .message(&message)
        .unwrap();
    assert_eq!(plaintext.into_verified().unwrap(), b"ring body");

    let plaintext = pgp
        .decrypt()
        .with_key(second)
        .verified_by(first.public_key())
        .message(&message)
        .unwrap();
    assert!(plaintext.signature().is_valid());
}

#[test]
fn user_id_change_keeps_key_material() {
    let key = key();
    assert_eq!(key.public_key().user_ids(), ["Rikka Test <test@proton.me>"]);

    let renamed = key
        .with_user_id("other@proton.me", "other@proton.me")
        .unwrap();

    assert_eq!(
        renamed.public_key().user_ids(),
        ["other@proton.me <other@proton.me>"]
    );
    assert_eq!(renamed.fingerprint(), key.fingerprint());
    let signature = Pgp::new()
        .sign()
        .with_key(renamed.clone())
        .detached(b"renamed")
        .unwrap();
    let reimported = PublicKey::import(renamed.public_key().export_armored().unwrap()).unwrap();
    assert!(
        Pgp::new()
            .verify()
            .with_key(reimported)
            .detached(b"renamed", signature)
            .is_valid()
    );
}

#[test]
fn post_quantum_key_is_v6() {
    let pgp = Pgp::new();
    let key = pgp
        .generate_key()
        .user_id("Rikka Test", "test@proton.me")
        .kind(KeyKind::PostQuantum)
        .generate()
        .unwrap();

    assert_eq!(key.version(), 6);
    assert_eq!(key.public_key().sha256_fingerprints().len(), 2);

    let unlocked = LockedKey::import(key.lock(PASSPHRASE).unwrap().export_armored().unwrap())
        .unwrap()
        .unlock(PASSPHRASE)
        .unwrap();

    let message = pgp
        .encrypt()
        .to(key.public_key())
        .signed_by(unlocked.clone())
        .message(b"post-quantum body")
        .unwrap();
    let plaintext = pgp
        .decrypt()
        .with_key(unlocked)
        .verified_by(key.public_key())
        .message(&message)
        .unwrap();
    assert_eq!(plaintext.data(), b"post-quantum body");
    assert!(plaintext.signature().is_valid());
}
