use std::os::unix::fs::PermissionsExt as _;

use proton_session::{Access, AccountStore, SavedAccount};
use rikka_proton_api::{Session, SessionStore as _};
use rikka_proton_cache::Table;
use rikka_secrets::blocking::Secrets;
use rikka_secrets::{Entry, schema};
use rikka_test_keyring::TestKeyring;

const FOLDERS: Table<Vec<String>> = Table::new("app.mail.folders");

fn saved_account() -> SavedAccount {
    SavedAccount {
        username: "person".to_owned(),
        passphrase: Some("the mailbox key".to_owned()),
    }
}

#[test]
fn saved_account_survives_reopen() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    store
        .save_account(&saved_account())
        .expect("failed to save account");

    let again = AccountStore::open_in(dir.path()).expect("failed to open store");
    let saved = again.saved_account().expect("saved account missing");

    assert_eq!(saved.username, "person");
    assert_eq!(saved.passphrase.as_deref(), Some("the mailbox key"));
}

#[test]
fn forget_removes_saved_account() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    store
        .save_account(&saved_account())
        .expect("failed to save account");

    store.forget();

    assert!(store.saved_account().is_none());
}

#[test]
fn lost_key_discards_saved_sign_in() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    store
        .save_account(&saved_account())
        .expect("failed to save account");
    store
        .sessions()
        .save(&Session::new("uid", "access", "refresh"))
        .expect("failed to save session");

    let removed = Secrets::shared()
        .expect("keyring is reachable")
        .remove(&Entry::new(schema::PROTON).attribute("store", dir.path().to_string_lossy()))
        .expect("key is removed");
    assert_eq!(removed, 1);
    let again = AccountStore::open_in(dir.path()).expect("failed to open store");

    assert!(again.saved_account().is_none());
    assert!(again.sessions().load().expect("session loads").is_none());
    Access::open(again).expect("failed to open access");
}

#[test]
fn store_key_stays_off_disk() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    store
        .save_account(&saved_account())
        .expect("failed to save account");

    let mode = std::fs::metadata(dir.path().join("account.sealed"))
        .expect("file missing")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("store folder is readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        names.iter().all(|name| !name.ends_with(".key")),
        "a key file reached the disk: {names:?}"
    );
}

#[test]
fn writes_after_forget_are_dropped() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let store = AccountStore::open_in(dir.path()).expect("failed to open store");
    let before = store.cache();
    before.put(&FOLDERS, "", vec!["Drafts".to_owned()]);

    store.forget();
    before.put(&FOLDERS, "", vec!["Inbox".to_owned()]);
    before.flush().expect("failed to flush cache");

    assert_eq!(store.cache().get(&FOLDERS, ""), None);
    drop(store);
    let reopened = AccountStore::open_in(dir.path()).expect("failed to reopen store");
    assert_eq!(reopened.cache().get(&FOLDERS, ""), None);
}
