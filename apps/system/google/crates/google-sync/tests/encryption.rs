use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use google_nigori::{Derivation, Nigori};
use google_sync::proto::{NigoriSpecifics, passphrase};
use google_sync::{Lock, client_tag_hash};
use prost::Message;

use crate::support::{FakeServer, keybag, login, passphrase_key, sync, synced};

const SALT: &[u8] = b"alpensalz";
const SCRYPT: i32 = 2;

fn custom_passphrase(key: &Nigori) -> NigoriSpecifics {
    NigoriSpecifics {
        encryption_keybag: Some(keybag(key, &[key])),
        keybag_is_frozen: Some(true),
        encrypt_everything: Some(true),
        passphrase_type: Some(passphrase::CUSTOM),
        custom_passphrase_key_derivation_method: Some(SCRYPT),
        custom_passphrase_key_derivation_salt: Some(STANDARD.encode(SALT)),
        ..NigoriSpecifics::default()
    }
}

#[test]
fn custom_passphrase_unlocks_stashed_passwords() {
    let key = passphrase_key("hunter2", SALT);
    let mut server = FakeServer::with_nigori(custom_passphrase(&key));
    let site = login("example.com", "rikka", "secret");
    let tag = client_tag_hash(&site);
    server.store_password(&tag, &key, &site.encode_to_vec());

    let mut engine = synced(&mut server);

    assert_eq!(engine.lock(), Lock::NeedsPassphrase);
    assert_eq!(engine.undecryptable(), 1);
    assert_eq!(engine.passwords().count(), 0);
    assert!(!engine.offer_passphrase("wrong").expect("an answer"));
    assert!(engine.offer_passphrase("hunter2").expect("an answer"));
    assert_eq!(engine.lock(), Lock::Open);
    assert_eq!(engine.undecryptable(), 0);
    assert_eq!(
        engine
            .password(&tag)
            .and_then(|found| found.password_value.as_deref()),
        Some("secret")
    );
}

#[test]
fn explicit_passphrase_commits_no_plain_metadata() {
    let key = passphrase_key("hunter2", SALT);
    let mut server = FakeServer::with_nigori(custom_passphrase(&key));
    let mut engine = synced(&mut server);
    engine.offer_passphrase("hunter2").expect("an answer");

    let tag = engine.put(login("example.com", "rikka", "secret"), None);
    sync(&mut engine, &mut server).expect("an upload");

    let password = server
        .stored(&tag)
        .and_then(|entity| entity.specifics.as_ref())
        .and_then(|specifics| specifics.password.clone())
        .expect("a stored password");
    assert!(password.unencrypted_metadata.is_none());
    let sealed = password.encrypted.expect("sealed data");
    assert_eq!(sealed.key_name, Some(key.name()));
    assert!(
        key.decrypt(sealed.blob.as_deref().unwrap_or_default())
            .is_some()
    );
}

#[test]
fn trusted_vault_keys_unlock_the_keybag() {
    let vault_key = b"a trusted vault key".to_vec();
    let wrapping = Nigori::derive(&Derivation::Pbkdf2, STANDARD.encode(&vault_key).as_bytes());
    let data_key = Nigori::derive(&Derivation::Pbkdf2, b"data key");
    let mut server = FakeServer::with_nigori(NigoriSpecifics {
        encryption_keybag: Some(keybag(&wrapping, &[&wrapping, &data_key])),
        passphrase_type: Some(passphrase::TRUSTED_VAULT),
        ..NigoriSpecifics::default()
    });
    let site = login("example.com", "rikka", "secret");
    server.store_password(&client_tag_hash(&site), &data_key, &site.encode_to_vec());

    let mut engine = synced(&mut server);

    assert_eq!(engine.lock(), Lock::NeedsVaultKeys);
    assert!(
        !engine
            .offer_vault_keys(&[b"stale".to_vec()])
            .expect("an answer")
    );
    assert!(engine.offer_vault_keys(&[vault_key]).expect("an answer"));
    assert_eq!(engine.passwords().count(), 1);
}

#[test]
fn keystore_keybag_from_another_client_opens() {
    let keystore = Nigori::derive(
        &Derivation::Pbkdf2,
        STANDARD.encode(crate::support::KEYSTORE_KEY).as_bytes(),
    );
    let data_key = Nigori::derive(&Derivation::Pbkdf2, b"an older implicit passphrase");
    let token = google_sync::proto::NigoriKey {
        encryption_key: Some(data_key.encryption_key().to_vec()),
        mac_key: Some(data_key.mac_key().to_vec()),
    };
    let mut server = FakeServer::with_nigori(NigoriSpecifics {
        encryption_keybag: Some(keybag(&data_key, &[&data_key, &keystore])),
        passphrase_type: Some(passphrase::KEYSTORE),
        keystore_decryptor_token: Some(crate::support::sealed(&keystore, &token.encode_to_vec())),
        ..NigoriSpecifics::default()
    });
    let site = login("example.com", "rikka", "secret");
    server.store_password(&client_tag_hash(&site), &data_key, &site.encode_to_vec());

    let engine = synced(&mut server);

    assert_eq!(engine.lock(), Lock::Open);
    assert_eq!(engine.passwords().count(), 1);
    assert!(server.commits.is_empty());
}

#[test]
fn notes_backup_fills_missing_notes() {
    let mut server = FakeServer::new();
    let mut author = synced(&mut server);
    let mut site = login("example.com", "rikka", "secret");
    site.notes = Some(google_sync::proto::Notes {
        note: vec![google_sync::proto::Note {
            value: Some("recovery codes".to_owned()),
            ..google_sync::proto::Note::default()
        }],
    });
    let tag = author.put(site, None);
    sync(&mut author, &mut server).expect("an upload");

    let reader = synced(&mut server);

    let notes = reader
        .password(&tag)
        .and_then(|found| found.notes.clone())
        .expect("notes");
    assert_eq!(notes.note[0].value.as_deref(), Some("recovery codes"));
}
