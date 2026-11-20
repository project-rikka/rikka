use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use rikka_keyring_vault::{Attributes, Error, Keyring, NewItem, Opener};
use tempfile::TempDir;

const PASSWORD: &[u8] = b"correct horse battery staple";
const TOKEN: &[u8] = b"notarealtoken-7f3a";

fn attributes(pairs: &[(&str, &str)]) -> Attributes {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn token_item(account: &str) -> NewItem<'static> {
    NewItem {
        label: "Discord sign-in",
        attributes: attributes(&[("xdg:schema", "os.rikka.Discord"), ("account", account)]),
        content_type: "text/plain",
        secret: TOKEN,
    }
}

fn login_at(folder: &TempDir) -> PathBuf {
    folder.path().join("keyrings").join("login.keyring")
}

fn reopened(path: &Path) -> Keyring {
    Keyring::open(path)
        .expect("keyring should be readable")
        .expect("keyring should exist")
}

#[test]
fn password_opens_stored_secrets() {
    let folder = TempDir::new().expect("temp folder");
    let path = login_at(&folder);
    let mut keyring =
        Keyring::create(&path, "Login", Opener::Password(PASSWORD)).expect("keyring is created");
    let stored = keyring
        .store(token_item("first"), true)
        .expect("item is stored");
    drop(keyring);

    let mut keyring = reopened(&path);
    assert!(keyring.is_locked());
    assert_eq!(keyring.label(), "Login");
    assert!(matches!(keyring.items(), Err(Error::Locked)));
    assert!(
        !keyring
            .unlock(Opener::Password(b"wrong"))
            .expect("unlock runs")
    );
    assert!(keyring.is_locked());
    assert!(
        keyring
            .unlock(Opener::Password(PASSWORD))
            .expect("unlock runs")
    );

    let item = keyring.item(stored.id).expect("item survives");
    assert_eq!(item.label(), "Discord sign-in");
    assert_eq!(item.attributes()["account"], "first");
    let secret = keyring.secret(stored.id).expect("secret opens");
    assert_eq!(secret.value.as_slice(), TOKEN);
    assert_eq!(secret.content_type, "text/plain");
}

#[test]
fn stored_file_is_private_and_opaque() {
    let folder = TempDir::new().expect("temp folder");
    let path = login_at(&folder);
    let mut keyring =
        Keyring::create(&path, "Login", Opener::Password(PASSWORD)).expect("keyring is created");
    keyring
        .store(token_item("someone@example.com"), true)
        .expect("item is stored");

    let file_mode = std::fs::metadata(&path).expect("file").permissions().mode();
    let folder_mode = std::fs::metadata(path.parent().expect("folder"))
        .expect("folder")
        .permissions()
        .mode();
    assert_eq!(file_mode & 0o777, 0o600);
    assert_eq!(folder_mode & 0o777, 0o700);

    let bytes = std::fs::read(&path).expect("file is readable");
    for revealing in [
        TOKEN,
        b"someone@example.com",
        b"os.rikka.Discord",
        b"Discord sign-in",
        PASSWORD,
    ] {
        assert!(
            !bytes
                .windows(revealing.len())
                .any(|window| window == revealing),
            "{} reached the disk",
            String::from_utf8_lossy(revealing)
        );
    }
}

type Alteration = fn(&mut serde_json::Value);

fn flip(encoded: &mut serde_json::Value) {
    let mut characters: Vec<char> = encoded.as_str().expect("base64 text").chars().collect();
    characters[4] = if characters[4] == 'A' { 'B' } else { 'A' };
    *encoded = characters.into_iter().collect::<String>().into();
}

#[test]
fn tampering_is_detected() {
    let folder = TempDir::new().expect("temp folder");
    let path = login_at(&folder);
    let mut keyring =
        Keyring::create(&path, "Login", Opener::Password(PASSWORD)).expect("keyring is created");
    keyring
        .store(token_item("first"), true)
        .expect("item is stored");
    drop(keyring);
    let pristine = std::fs::read(&path).expect("file is readable");

    let edits: [(&str, Alteration); 3] = [
        ("contents", |file| flip(&mut file["content"]["sealed"])),
        ("creation time", |file| {
            let created = file["content"]["created"].as_u64().expect("created");
            file["content"]["created"] = (created + 1).into();
        }),
        ("slot", |file| {
            flip(&mut file["content"]["slots"][0]["sealed"])
        }),
    ];
    for (part, edit) in edits {
        let mut file: serde_json::Value = serde_json::from_slice(&pristine).expect("json");
        edit(&mut file);
        std::fs::write(&path, serde_json::to_vec(&file).expect("json")).expect("file rewritten");

        let mut keyring = reopened(&path);
        let opened = keyring.unlock(Opener::Password(PASSWORD));
        assert!(
            matches!(opened, Err(Error::Tampered { .. }) | Ok(false)),
            "altered {part} was accepted"
        );
        assert!(keyring.is_locked(), "altered {part} unlocked the keyring");
    }
}

#[test]
fn new_password_replaces_old() {
    let folder = TempDir::new().expect("temp folder");
    let path = login_at(&folder);
    let mut keyring =
        Keyring::create(&path, "Login", Opener::Password(PASSWORD)).expect("keyring is created");
    let stored = keyring
        .store(token_item("first"), true)
        .expect("item is stored");

    keyring
        .set_password(b"a new login password")
        .expect("password changes");
    assert!(
        keyring
            .opens_with(b"a new login password")
            .expect("check runs")
    );
    drop(keyring);

    let mut keyring = reopened(&path);
    assert!(
        !keyring
            .unlock(Opener::Password(PASSWORD))
            .expect("unlock runs")
    );
    assert!(
        keyring
            .unlock(Opener::Password(b"a new login password"))
            .expect("unlock runs")
    );
    assert_eq!(
        keyring
            .secret(stored.id)
            .expect("secret opens")
            .value
            .as_slice(),
        TOKEN
    );
}

#[test]
fn wrapped_keyring_opens_through_login() {
    let folder = TempDir::new().expect("temp folder");
    let login_path = login_at(&folder);
    let other_path = folder.path().join("keyrings").join("work.keyring");
    let login =
        Keyring::create(&login_path, "Login", Opener::Password(PASSWORD)).expect("login created");
    let mut other =
        Keyring::create(&other_path, "Work", Opener::Keyring(&login)).expect("work created");
    other
        .store(token_item("work"), true)
        .expect("item is stored");
    drop((login, other));

    let mut login = reopened(&login_path);
    let mut other = reopened(&other_path);
    assert!(matches!(
        other.unlock(Opener::Keyring(&login)),
        Err(Error::Locked)
    ));
    assert!(
        !other
            .unlock(Opener::Password(PASSWORD))
            .expect("unlock runs")
    );
    assert!(
        login
            .unlock(Opener::Password(PASSWORD))
            .expect("unlock runs")
    );
    assert!(other.unlock(Opener::Keyring(&login)).expect("unlock runs"));
    assert_eq!(other.items().expect("unlocked").len(), 1);
}

#[test]
fn binding_opens_without_password() {
    let folder = TempDir::new().expect("temp folder");
    let path = login_at(&folder);
    let mut keyring =
        Keyring::create(&path, "Login", Opener::Password(PASSWORD)).expect("keyring is created");
    keyring
        .bind("systemd-creds", |slot_key| {
            Ok::<_, std::convert::Infallible>(slot_key.to_vec())
        })
        .expect("binding is added");
    drop(keyring);

    let mut keyring = reopened(&path);
    let binding = keyring.binding().expect("binding is kept");
    assert_eq!(binding.name, "systemd-creds");
    let slot_key = binding.blob.to_vec();
    assert!(!keyring.unlock_bound(&[7; 32]).expect("unlock runs"));
    assert!(keyring.unlock_bound(&slot_key).expect("unlock runs"));

    keyring.lock();
    assert!(keyring.unbind().expect("binding is removed"));
    drop(keyring);
    let mut keyring = reopened(&path);
    assert!(keyring.binding().is_none());
    assert!(!keyring.unlock_bound(&slot_key).expect("unlock runs"));
    assert!(
        keyring
            .unlock(Opener::Password(PASSWORD))
            .expect("password still opens")
    );
}

#[test]
fn concurrent_handles_keep_each_others_writes() {
    let folder = TempDir::new().expect("temp folder");
    let path = login_at(&folder);
    let mut first =
        Keyring::create(&path, "Login", Opener::Password(PASSWORD)).expect("keyring is created");
    let mut second = reopened(&path);
    assert!(
        second
            .unlock(Opener::Password(PASSWORD))
            .expect("unlock runs")
    );

    first
        .store(token_item("first"), true)
        .expect("first stores");
    second
        .store(token_item("second"), true)
        .expect("second stores");
    first.refresh().expect("first refreshes");

    for keyring in [&first, &second] {
        let mut accounts: Vec<String> = keyring
            .items()
            .expect("unlocked")
            .iter()
            .map(|item| item.attributes()["account"].clone())
            .collect();
        accounts.sort();
        assert_eq!(accounts, ["first", "second"]);
    }
}

#[test]
fn replace_updates_matching_item() {
    let folder = TempDir::new().expect("temp folder");
    let mut keyring = Keyring::create(login_at(&folder), "Login", Opener::Password(PASSWORD))
        .expect("keyring is created");
    let first = keyring
        .store(token_item("first"), true)
        .expect("item is stored");
    let renewed = keyring
        .store(
            NewItem {
                secret: b"renewed",
                ..token_item("first")
            },
            true,
        )
        .expect("item is replaced");
    let duplicate = keyring
        .store(token_item("first"), false)
        .expect("item is added");

    assert!(!first.replaced);
    assert!(renewed.replaced);
    assert_eq!(renewed.id, first.id);
    assert_ne!(duplicate.id, first.id);
    assert_eq!(
        keyring
            .secret(first.id)
            .expect("secret opens")
            .value
            .as_slice(),
        b"renewed"
    );
    let found = keyring
        .search(&attributes(&[("xdg:schema", "os.rikka.Discord")]))
        .expect("unlocked");
    assert_eq!(found.len(), 2);
    assert!(
        keyring
            .search(&attributes(&[("account", "other")]))
            .expect("unlocked")
            .is_empty()
    );

    keyring.delete_item(first.id).expect("item is deleted");
    assert!(matches!(
        keyring.secret(first.id),
        Err(Error::NoSuchItem(_))
    ));
}
