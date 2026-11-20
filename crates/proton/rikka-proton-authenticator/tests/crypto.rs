use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_account::UserKeys;
use rikka_proton_authenticator::{AuthenticatorKey, Entry, Error};
use rikka_proton_pgp::{Clock, KeyRing, Pgp, PrivateKey};

const URI: &str = "otpauth://totp/Proton%3Aperson?secret=JBSWY3DPEHPK3PXP&issuer=Proton\
&algorithm=SHA1&digits=6&period=30";

fn pgp() -> Pgp {
    Pgp::with_clock(Clock::fixed(1_700_000_000))
}

fn user_key(pgp: &Pgp) -> PrivateKey {
    pgp.generate_key()
        .user_id("tester", "person@proton.me")
        .generate()
        .unwrap()
}

fn user_keys(key: &PrivateKey) -> UserKeys {
    UserKeys::new(KeyRing::new().with_private(key.clone()), key.clone())
}

#[test]
fn authenticator_key_round_trips() {
    let pgp = pgp();
    let keys = user_keys(&user_key(&pgp));
    let key = AuthenticatorKey::generate();

    let wrapped = key.wrap(&pgp, &keys).unwrap();
    let decrypted = AuthenticatorKey::unwrap(&pgp, &keys, &wrapped).unwrap();

    assert_eq!(decrypted.as_bytes(), key.as_bytes());
    assert_eq!(decrypted.as_bytes().len(), 32);
    assert!(BASE64_STANDARD.decode(&wrapped).is_ok());
    assert!(!wrapped.contains(&BASE64_STANDARD.encode(key.as_bytes())));
}

#[test]
fn rejects_foreign_authenticator_key() {
    let pgp = pgp();
    let wrapped = AuthenticatorKey::generate()
        .wrap(&pgp, &user_keys(&user_key(&pgp)))
        .unwrap();

    let error = AuthenticatorKey::unwrap(&pgp, &user_keys(&user_key(&pgp)), &wrapped).unwrap_err();

    assert!(matches!(error, Error::Pgp(_)));
}

#[test]
fn rejects_unsigned_authenticator_key() {
    let pgp = pgp();
    let key = user_key(&pgp);
    let keys = user_keys(&key);

    let message = pgp
        .encrypt()
        .to(key.public_key())
        .message(AuthenticatorKey::generate().as_bytes())
        .unwrap();

    let error =
        AuthenticatorKey::unwrap(&pgp, &keys, &BASE64_STANDARD.encode(message)).unwrap_err();

    assert!(matches!(error, Error::InvalidKeySignature { .. }));
}

#[test]
fn sealed_entry_round_trips() {
    let key = AuthenticatorKey::generate();
    let entry = Entry::from_uri("entry-one", URI)
        .unwrap()
        .with_note("the note");

    let content = key.seal(&entry).unwrap();
    let decrypted = key.open(&content).unwrap();

    assert_eq!(decrypted, entry);
    assert_eq!(decrypted.id, "entry-one");
    assert_eq!(decrypted.note.as_deref(), Some("the note"));
    assert_eq!(decrypted.issuer(), Some("Proton"));
    assert_eq!(decrypted.secret(), "JBSWY3DPEHPK3PXP");

    let at = jiff::Timestamp::from_second(1_700_000_000).unwrap();
    assert_eq!(decrypted.code_at(at).unwrap(), entry.code_at(at).unwrap());
    assert_eq!(decrypted.code_at(at).unwrap().len(), 6);
}

#[test]
fn sealed_entry_nonce_is_unique() {
    let key = AuthenticatorKey::generate();
    let entry = Entry::from_uri("entry-one", URI).unwrap();

    let once = key.seal(&entry).unwrap();
    let twice = key.seal(&entry).unwrap();

    assert_ne!(once, twice);
    assert_eq!(
        BASE64_STANDARD.decode(&once).unwrap().len(),
        12 + 16 + entry.encode().len()
    );
}
