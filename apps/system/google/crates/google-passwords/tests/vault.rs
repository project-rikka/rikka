use std::path::Path;
use std::sync::Arc;

use google_account::{Endpoint, Grants, Master};
use google_passwords::{Draft, Error, Lock, Refresh, Seal, Server, Vault};
use google_test_sync::{FakeServer, Google};

async fn vault(google: &Google, folder: &Path, seal: &Seal) -> Vault {
    let endpoint = Endpoint::new(&google.auth()).expect("a local endpoint");
    let master = Master::sign_in(&endpoint, "oauth2_4/code")
        .await
        .expect("a master token");
    let grants = Arc::new(Grants::new(endpoint, master));
    let server = Server::new(rikka_net::Client::new(), &google.sync()).expect("a sync server");
    Vault::open(folder, grants, server, seal.clone()).expect("a vault")
}

fn draft(address: &str, username: &str, password: &str) -> Draft {
    Draft {
        address: address.to_owned(),
        username: username.to_owned(),
        password: password.to_owned(),
        note: String::new(),
    }
}

#[tokio::test]
async fn saved_login_reaches_another_device_over_http() {
    let google = Google::start(FakeServer::new());
    let (phone_folder, desktop_folder) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let phone = vault(&google, phone_folder.path(), &Seal::generate()).await;
    let desktop = vault(&google, desktop_folder.path(), &Seal::generate()).await;
    phone.sync(None).await.expect("a first sync");

    let id = phone
        .save(&draft("example.com/login", " rikka ", "hunter2"), None)
        .expect("a saved login");
    phone.sync(None).await.expect("an upload");
    let status = desktop.sync(None).await.expect("a download");

    assert_eq!(status.lock, Lock::Open);
    assert_eq!(status.unsynced, 0);
    let seen = desktop.credentials();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].id, id);
    assert_eq!(seen[0].site.realm, "https://example.com/");
    assert_eq!(seen[0].site.address, "https://example.com/login");
    assert_eq!(seen[0].username, "rikka");
    assert_eq!(seen[0].password, "hunter2");
    assert!(seen[0].changed_ms.is_some());
    let command = google
        .requests()
        .into_iter()
        .find(|request| request.path() == "/chrome-sync/command/")
        .expect("a sync command");
    assert_eq!(command.query()["client"], "Google Chrome");
    assert_eq!(command.query()["client_id"].len(), 24);
    assert!(
        command
            .header("user-agent")
            .unwrap_or_default()
            .starts_with("Chrome ")
    );
}

#[tokio::test]
async fn edits_and_deletions_follow_the_saved_login() {
    let google = Google::start(FakeServer::new());
    let (first, second) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let author = vault(&google, first.path(), &Seal::generate()).await;
    let reader = vault(&google, second.path(), &Seal::generate()).await;
    author.sync(None).await.expect("a first sync");
    let id = author
        .save(&draft("https://example.com", "rikka", "one"), None)
        .expect("a saved login");
    author.sync(None).await.expect("an upload");

    let mut edited = Draft::from_credential(&author.credentials()[0]);
    edited.password = "two".to_owned();
    edited.note = "changed on the phone".to_owned();
    assert_eq!(author.save(&edited, Some(&id)).expect("an edit"), id);
    author.sync(None).await.expect("an upload");
    reader.sync(None).await.expect("a download");
    assert_eq!(reader.credentials()[0].password, "two");
    assert_eq!(reader.credentials()[0].note, "changed on the phone");

    reader.delete(&id).expect("a deletion");
    reader.sync(None).await.expect("an upload");
    author
        .sync(Some(Refresh::Trigger))
        .await
        .expect("a download");
    assert!(author.credentials().is_empty());
}

#[tokio::test]
async fn sealed_snapshot_survives_a_restart_and_needs_its_seal() {
    let google = Google::start(FakeServer::new());
    let folder = tempfile::tempdir().unwrap();
    let seal = Seal::generate();
    let first = vault(&google, folder.path(), &seal).await;
    first.sync(None).await.expect("a first sync");
    first
        .save(&draft("example.com", "rikka", "hunter2"), None)
        .expect("a saved login");
    drop(first);

    let sealed = std::fs::read(folder.path().join("passwords.sealed")).expect("a snapshot");
    assert!(!sealed.windows(7).any(|window| window == b"hunter2"));
    let reopened = vault(&google, folder.path(), &seal).await;
    assert_eq!(reopened.credentials().len(), 1);
    assert_eq!(reopened.status().unsynced, 1);
    assert!(reopened.status().has_synced);

    let foreign = vault(&google, folder.path(), &Seal::generate()).await;
    assert!(foreign.credentials().is_empty());
    assert!(!foreign.status().has_synced);
}

#[tokio::test]
async fn drafts_without_a_site_or_password_are_refused() {
    let google = Google::start(FakeServer::new());
    let folder = tempfile::tempdir().unwrap();
    let vault = vault(&google, folder.path(), &Seal::generate()).await;

    let refusals = [
        (draft("ftp://example.com", "rikka", "secret"), "address"),
        (draft("", "rikka", "secret"), "address"),
        (draft("example.com", "rikka", ""), "password"),
    ];
    for (draft, reason) in refusals {
        let refused = vault.save(&draft, None);
        match reason {
            "address" => assert!(matches!(refused, Err(Error::Address))),
            _ => assert!(matches!(refused, Err(Error::EmptyPassword))),
        }
    }
    assert!(matches!(
        vault.save(&draft("example.com", "rikka", "secret"), Some("gone")),
        Err(Error::Missing)
    ));
}
