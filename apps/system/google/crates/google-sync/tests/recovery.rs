use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use google_nigori::{Derivation, Nigori};
use google_sync::proto::{ClientToServerResponse, PasswordSpecificsData};
use google_sync::{Error, Refresh, client_tag_hash};
use prost::Message;

use crate::support::{FakeServer, KEYSTORE_KEY, login, sync, synced};

const THROTTLED: i32 = 3;
const UNKNOWN_FIELD: [u8; 5] = [0x9a, 0x06, 0x02, b'o', b'k'];

#[test]
fn conflicting_commit_takes_the_remote_change() {
    let mut server = FakeServer::new();
    let mut first = synced(&mut server);
    let site = login("example.com", "rikka", "original");
    let tag = first.put(site.clone(), None);
    sync(&mut first, &mut server).expect("an upload");
    let mut second = synced(&mut server);

    let mut remote = site.clone();
    remote.password_value = Some("remote".to_owned());
    first.put(remote, Some(&tag));
    sync(&mut first, &mut server).expect("an upload");
    let mut local = site;
    local.password_value = Some("local".to_owned());
    second.put(local, Some(&tag));
    sync(&mut second, &mut server).expect("a resolved conflict");

    assert_eq!(second.unsynced(), 0);
    assert_eq!(
        second
            .password(&tag)
            .and_then(|found| found.password_value.as_deref()),
        Some("remote")
    );
}

#[test]
fn restarted_server_keeps_unsent_logins() {
    let mut server = FakeServer::new();
    let mut engine = synced(&mut server);
    engine.put(login("synced.example", "rikka", "secret"), None);
    sync(&mut engine, &mut server).expect("an upload");
    let guid = engine.cache_guid().to_owned();
    let unsent = engine.put(login("unsent.example", "rikka", "secret"), None);
    server.start_over();

    let reset = sync(&mut engine, &mut server);
    assert!(matches!(reset, Err(Error::StoreReset)));
    sync(&mut engine, &mut server).expect("a fresh sync");

    assert_ne!(engine.cache_guid(), guid);
    assert_eq!(engine.passwords().count(), 1);
    assert!(server.stored(&unsent).is_some());
    assert_eq!(engine.unsynced(), 0);
}

#[test]
fn unknown_fields_survive_a_local_edit() {
    let mut server = FakeServer::new();
    synced(&mut server);
    let key = Nigori::derive(
        &Derivation::Pbkdf2,
        STANDARD.encode(KEYSTORE_KEY).as_bytes(),
    );
    let site = login("example.com", "rikka", "secret");
    let tag = client_tag_hash(&site);
    let mut plain = site.encode_to_vec();
    plain.extend_from_slice(&UNKNOWN_FIELD);
    server.store_password(&tag, &key, &plain);
    let mut engine = synced(&mut server);

    let mut changed = site;
    changed.password_value = Some("changed".to_owned());
    engine.put(changed, Some(&tag));
    sync(&mut engine, &mut server).expect("an upload");

    let sealed = server
        .stored(&tag)
        .and_then(|entity| entity.specifics.as_ref())
        .and_then(|specifics| specifics.password.as_ref())
        .and_then(|password| password.encrypted.clone())
        .expect("sealed data");
    let committed = key
        .decrypt(sealed.blob.as_deref().unwrap_or_default())
        .expect("a readable commit");
    assert!(committed.ends_with(&UNKNOWN_FIELD));
    let decoded = PasswordSpecificsData::decode(committed.as_slice()).expect("a password");
    assert_eq!(decoded.password_value.as_deref(), Some("changed"));
}

#[test]
fn remote_deletion_yields_to_a_local_edit() {
    let mut server = FakeServer::new();
    let mut first = synced(&mut server);
    let site = login("example.com", "rikka", "original");
    let tag = first.put(site.clone(), None);
    sync(&mut first, &mut server).expect("an upload");
    let mut second = synced(&mut server);
    first.remove(&tag);
    sync(&mut first, &mut server).expect("a deletion");

    let mut edited = site;
    edited.password_value = Some("kept".to_owned());
    second.put(edited, Some(&tag));
    sync(&mut second, &mut server).expect("an upload");

    assert_eq!(second.passwords().count(), 1);
    assert_eq!(
        server.stored(&tag).and_then(|entity| entity.deleted),
        Some(false)
    );
}

#[test]
fn throttling_reports_the_delay_and_keeps_changes() {
    let mut server = FakeServer::new();
    let mut engine = synced(&mut server);
    engine.put(login("example.com", "rikka", "secret"), None);
    engine.next_request().expect("a commit");

    let throttled = engine.receive(ClientToServerResponse {
        error_code: Some(THROTTLED),
        ..ClientToServerResponse::default()
    });

    assert!(matches!(throttled, Err(Error::Throttled(_))));
    assert_eq!(engine.unsynced(), 1);
    engine.request_refresh(Refresh::Trigger);
    sync(&mut engine, &mut server).expect("a later upload");
    assert_eq!(engine.unsynced(), 0);
}
