use google_sync::proto::{PasswordSpecificsData, passphrase};
use google_sync::{Engine, Lock, PassphraseType, client_tag_hash};

use crate::support::{ACCOUNT, FakeServer, login, sync, synced};

fn secret<'a>(engine: &'a Engine, data: &PasswordSpecificsData) -> Option<&'a str> {
    engine
        .password(&client_tag_hash(data))
        .and_then(|found| found.password_value.as_deref())
}

#[test]
fn first_device_initializes_keystore_encryption() {
    let mut server = FakeServer::new();

    let engine = synced(&mut server);

    assert_eq!(engine.lock(), Lock::Open);
    assert_eq!(engine.passphrase_type(), Some(PassphraseType::Keystore));
    let nigori = server.commits[0]
        .specifics
        .as_ref()
        .and_then(|specifics| specifics.nigori.as_ref())
        .expect("a committed nigori");
    assert_eq!(nigori.passphrase_type, Some(passphrase::KEYSTORE));
    assert!(nigori.keystore_decryptor_token.is_some());
    assert_eq!(server.commits[0].folder, Some(true));
}

#[test]
fn changes_travel_between_two_devices() {
    let mut server = FakeServer::new();
    let mut phone = synced(&mut server);
    let mut desktop = synced(&mut server);
    let site = login("example.com", "rikka", "first");

    let tag = phone.put(site.clone(), None);
    sync(&mut phone, &mut server).expect("an upload");
    desktop.request_refresh(google_sync::Refresh::Trigger);
    sync(&mut desktop, &mut server).expect("a download");
    assert_eq!(secret(&desktop, &site), Some("first"));
    assert_eq!(phone.unsynced(), 0);

    let mut changed = site.clone();
    changed.password_value = Some("second".to_owned());
    desktop.put(changed, Some(&tag));
    sync(&mut desktop, &mut server).expect("an upload");
    phone.request_refresh(google_sync::Refresh::Trigger);
    sync(&mut phone, &mut server).expect("a download");
    assert_eq!(secret(&phone, &site), Some("second"));

    phone.remove(&tag);
    sync(&mut phone, &mut server).expect("a deletion");
    desktop.request_refresh(google_sync::Refresh::Periodic);
    sync(&mut desktop, &mut server).expect("a download");
    assert_eq!(desktop.passwords().count(), 0);
    assert_eq!(
        server.stored(&tag).and_then(|entity| entity.deleted),
        Some(true)
    );
}

#[test]
fn committed_password_is_sealed_with_plain_metadata_only() {
    let mut server = FakeServer::new();
    let mut engine = synced(&mut server);

    let tag = engine.put(login("example.com", "rikka", "hunter2"), None);
    sync(&mut engine, &mut server).expect("an upload");

    let stored = server.stored(&tag).expect("a stored password");
    let password = stored
        .specifics
        .as_ref()
        .and_then(|specifics| specifics.password.as_ref())
        .expect("password specifics");
    let wire = prost::Message::encode_to_vec(stored);
    assert!(!wire.windows(7).any(|window| window == b"hunter2"));
    assert_eq!(stored.name.as_deref(), Some("encrypted"));
    assert_eq!(
        password
            .unencrypted_metadata
            .as_ref()
            .and_then(|metadata| metadata.url.as_deref()),
        Some("https://example.com/")
    );
    assert!(password.encrypted_notes_backup.is_some());
}

#[test]
fn renamed_login_replaces_its_entity() {
    let mut server = FakeServer::new();
    let mut engine = synced(&mut server);
    let old = engine.put(login("example.com", "old", "secret"), None);
    sync(&mut engine, &mut server).expect("an upload");

    let new = engine.put(login("example.com", "new", "secret"), Some(&old));
    sync(&mut engine, &mut server).expect("an upload");

    assert_ne!(old, new);
    assert_eq!(
        server.stored(&old).and_then(|entity| entity.deleted),
        Some(true)
    );
    assert_eq!(
        server.stored(&new).and_then(|entity| entity.deleted),
        Some(false)
    );
    assert_eq!(engine.passwords().count(), 1);
}

#[test]
fn paged_downloads_arrive_completely() {
    let mut server = FakeServer::new();
    let mut author = synced(&mut server);
    for index in 0..7 {
        author.put(
            login(&format!("site{index}.example"), "rikka", "secret"),
            None,
        );
    }
    sync(&mut author, &mut server).expect("an upload");
    server.page = 3;

    let reader = synced(&mut server);

    assert_eq!(reader.passwords().count(), 7);
}

#[test]
fn restored_engine_continues_where_it_stopped() {
    let mut server = FakeServer::new();
    let mut engine = synced(&mut server);
    engine.put(login("example.com", "rikka", "secret"), None);
    sync(&mut engine, &mut server).expect("an upload");
    let pending = engine.put(login("pending.example", "rikka", "later"), None);

    let mut restored = Engine::restore(ACCOUNT, &engine.snapshot()).expect("a snapshot");

    assert_eq!(restored.cache_guid(), engine.cache_guid());
    assert_eq!(restored.passwords().count(), 2);
    assert_eq!(restored.unsynced(), 1);
    let before = server.requests;
    sync(&mut restored, &mut server).expect("an upload");
    assert_eq!(server.requests, before + 1);
    assert!(server.stored(&pending).is_some());
}
