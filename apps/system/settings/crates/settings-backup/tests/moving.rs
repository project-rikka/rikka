use rikka_backup::Bundle;
use rikka_secrets::blocking::Secrets;
use rikka_secrets::{Entry, schema};
use rikka_test_keyring::TestKeyring;

use crate::support::{bundle, dirs};

const PASSPHRASE: &str = "a long passphrase";

#[test]
fn moves_sign_ins_to_the_restoring_computer() {
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };
    let secrets = Secrets::shared().unwrap();
    let old = tempfile::tempdir().unwrap();
    let folder = old.path().join(".local/state/rikka/discord/accounts/7");
    let discord = Entry::new(schema::DISCORD).attribute("folder", folder.to_str().unwrap());
    let llm = Entry::new(schema::LLM).attribute("backend", "anthropic");
    let unrelated = Entry::new("org.example.Other");
    secrets
        .store(&discord, "Discord sign-in", b"token")
        .unwrap();
    secrets
        .store(&llm, "Claude sign-in", b"{\"key\":1}")
        .unwrap();
    secrets.store(&unrelated, "Other", b"kept").unwrap();

    let mut exported = bundle(old.path());
    exported.credentials = settings_backup::collect_credentials(&secrets).unwrap();
    assert_eq!(exported.credentials.len(), 2);
    let target = old.path().join("backup.rikka-backup");
    settings_backup::export(&exported, PASSPHRASE, &target).unwrap();

    assert!(!secrets.contains(&discord).unwrap());
    assert!(!secrets.contains(&llm).unwrap());
    assert!(secrets.contains(&unrelated).unwrap());

    let opened = Bundle::open(std::fs::File::open(&target).unwrap(), PASSPHRASE).unwrap();
    let new = tempfile::tempdir().unwrap();
    settings_backup::restore(&opened, &dirs(new.path())).unwrap();

    let moved = new.path().join(".local/state/rikka/discord/accounts/7");
    let restored = Entry::new(schema::DISCORD).attribute("folder", moved.to_str().unwrap());
    assert_eq!(
        secrets.lookup(&restored).unwrap().unwrap().expose(),
        b"token"
    );
    assert_eq!(
        secrets.lookup(&llm).unwrap().unwrap().expose(),
        b"{\"key\":1}"
    );
    assert!(moved.is_dir());
}
