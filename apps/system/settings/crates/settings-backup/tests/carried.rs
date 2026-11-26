use std::path::PathBuf;

use rikka_backup::Base;

use crate::support::{bundle, dirs, write};

#[test]
fn carries_account_records_and_sealed_sessions_but_not_caches() {
    let old = tempfile::tempdir().unwrap();
    let from = dirs(old.path());
    write(
        &from.state.join("discord/accounts.json"),
        "{\"accounts\":[]}",
    );
    write(&from.state.join("discord/accounts/1/store.sqlite"), "cache");
    write(&from.state.join("proton/accounts.sealed"), "a");
    write(&from.state.join("proton/0123/session.sealed"), "s");
    write(&from.state.join("proton/0123/cache/cache.sealed"), "c");
    write(&from.state.join("proton/0123/cache.sqlite"), "c");
    write(&from.config.join("remote-viewer/hosts.json"), "[]");

    let files = settings_backup::collect_files(&from).unwrap();
    let carried: Vec<(Base, PathBuf)> = files
        .iter()
        .map(|file| (file.base(), file.path().to_owned()))
        .collect();
    assert_eq!(
        carried,
        [
            (Base::State, PathBuf::from("discord/accounts.json")),
            (Base::State, PathBuf::from("proton/0123/session.sealed")),
            (Base::State, PathBuf::from("proton/accounts.sealed")),
            (Base::Config, PathBuf::from("remote-viewer/hosts.json")),
        ]
    );

    let new = tempfile::tempdir().unwrap();
    let to = dirs(new.path());
    let mut restoring = bundle(old.path());
    restoring.files = files;
    settings_backup::restore(&restoring, &to).unwrap();
    assert_eq!(
        std::fs::read_to_string(to.state.join("proton/0123/session.sealed")).unwrap(),
        "s"
    );
    assert_eq!(
        std::fs::read_to_string(to.config.join("remote-viewer/hosts.json")).unwrap(),
        "[]"
    );
    assert!(!to.state.join("discord/accounts/1").exists());
}
