use futures_util::StreamExt;
use rikka_secrets::{Change, Entry, Secrets, blocking};
use rikka_test_bus::TIMEOUT;
use rikka_test_keyring::TestKeyring;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Tokens {
    access: String,
    refresh: Option<String>,
}

fn account(name: &str) -> Entry {
    Entry::new("os.rikka.Test").attribute("account", name)
}

#[test]
fn stored_json_reads_back_and_replaces() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let secrets = zbus::block_on(Secrets::connect()).expect("client connects");
    let first = Tokens {
        access: "notarealtoken".to_owned(),
        refresh: Some("notarealrefresh".to_owned()),
    };
    let renewed = Tokens {
        access: "renewed".to_owned(),
        refresh: None,
    };

    zbus::block_on(secrets.store_json(&account("json"), "Test sign-in", &first)).expect("stores");
    zbus::block_on(secrets.store_json(&account("json"), "Test sign-in", &renewed))
        .expect("replaces");

    let read: Tokens = zbus::block_on(secrets.lookup_json(&account("json")))
        .expect("lookup runs")
        .expect("tokens are found");
    assert_eq!(read, renewed);
    let found = zbus::block_on(secrets.search(&Entry::new("os.rikka.Test"))).expect("search runs");
    assert_eq!(
        found
            .iter()
            .filter(|found| found.attributes.get("account").map(String::as_str) == Some("json"))
            .count(),
        1
    );
}

#[test]
fn remove_deletes_every_match() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let secrets = blocking::Secrets::shared().expect("client connects");
    for kind in ["cookies", "session"] {
        secrets
            .store(
                &account("removed").attribute("kind", kind),
                "Test",
                b"secret",
            )
            .expect("stores");
    }

    assert_eq!(secrets.remove(&account("removed")).expect("removes"), 2);
    assert!(
        secrets
            .lookup(&account("removed"))
            .expect("lookup runs")
            .is_none()
    );
    assert!(
        zbus::block_on(async { Secrets::shared().await?.items().await })
            .expect("items list")
            .iter()
            .all(|item| item.attributes.get("account").map(String::as_str) != Some("removed"))
    );
}

#[test]
fn changes_announce_stored_items() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let secrets = zbus::block_on(Secrets::connect()).expect("client connects");
    let mut changes = zbus::block_on(secrets.changes()).expect("changes are watched");
    let (sender, announced) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(zbus::block_on(changes.next()));
    });

    zbus::block_on(secrets.store(&account("announced"), "Test", b"secret")).expect("stores");

    assert_eq!(
        announced
            .recv_timeout(TIMEOUT)
            .expect("a change should be announced"),
        Some(Change::Items)
    );
}
