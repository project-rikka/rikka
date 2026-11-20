use std::path::PathBuf;

use rikka_settings::{Error, Key, Layers, LoadReport, Schema, Source, Store};
use tempfile::TempDir;

fn schema() -> Schema {
    Schema::new("com.example.calculator")
        .key(Key::bool("scientific", false))
        .key(Key::int("precision", 10).range(0..=20))
        .key(Key::string("angle-unit", "degrees").choices(["degrees", "radians"]))
}

fn store() -> (Store, TempDir, TempDir) {
    let system = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let layers = Layers::new(system.path(), user.path());
    let (store, report) = Store::load(schema(), layers).unwrap();
    assert!(report.is_empty());
    (store, system, user)
}

fn put(directory: &TempDir, contents: &str) {
    std::fs::write(
        directory.path().join("com.example.calculator.toml"),
        contents,
    )
    .unwrap();
}

fn reload(system: &TempDir, user: &TempDir) -> (Store, LoadReport) {
    Store::load(schema(), Layers::new(system.path(), user.path())).unwrap()
}

fn store_that_cannot_write() -> (Store, TempDir, TempDir, PathBuf) {
    let system = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let (store, report) = Store::load(schema(), Layers::new(system.path(), &config)).unwrap();
    assert!(report.is_empty());
    std::fs::write(&config, "not a directory").unwrap();
    (store, system, root, config)
}

#[test]
fn defaults_resolve_without_files() {
    let (store, ..) = store();
    assert!(!store.get::<bool>("scientific").unwrap());
    assert_eq!(store.get::<i64>("precision").unwrap(), 10);
    assert_eq!(store.source("precision").unwrap(), Source::Default);
}

#[test]
fn layers_resolve_in_order() {
    let (_, system, user) = store();
    put(&system, "precision = 4\nscientific = true\n");
    let (store, report) = reload(&system, &user);
    assert!(report.is_empty());
    assert_eq!(store.get::<i64>("precision").unwrap(), 4);
    assert_eq!(store.source("precision").unwrap(), Source::System);

    put(&user, "precision = 12\n");
    let (store, _) = reload(&system, &user);
    assert_eq!(store.get::<i64>("precision").unwrap(), 12);
    assert_eq!(store.source("precision").unwrap(), Source::User);
    assert!(
        store.get::<bool>("scientific").unwrap(),
        "the vendor still answers for what the user did not"
    );
}

#[test]
fn reset_reveals_lower_layer() {
    let (_, system, user) = store();
    put(&system, "precision = 4\n");
    let (mut store, _) = reload(&system, &user);

    store.set("precision", 12).unwrap();
    assert_eq!(store.get::<i64>("precision").unwrap(), 12);

    store.reset("precision").unwrap();
    assert_eq!(store.get::<i64>("precision").unwrap(), 4);
    assert_eq!(store.source("precision").unwrap(), Source::System);
}

#[test]
fn write_persists_and_reads_back() {
    let (mut store, system, user) = store();
    store.set("angle-unit", "radians").unwrap();
    rikka_settings::flush_pending_writes();

    let (read_back, _) = reload(&system, &user);
    assert_eq!(read_back.get::<String>("angle-unit").unwrap(), "radians");
}

#[test]
fn empty_user_file_is_removed() {
    let (mut store, _system, user) = store();
    let path = user.path().join("com.example.calculator.toml");

    store.set("precision", 12).unwrap();
    rikka_settings::flush_pending_writes();
    assert!(path.is_file());

    store.reset("precision").unwrap();
    rikka_settings::flush_pending_writes();
    assert!(!path.exists());
}

#[test]
fn volatile_layer_keeps_changes_in_memory() {
    let (_, system, user) = store();
    put(&user, "precision = 12\n");
    let layers = Layers::new(system.path(), user.path()).volatile();
    let (mut store, _) = Store::load(schema(), layers.clone()).unwrap();
    assert_eq!(store.get::<i64>("precision").unwrap(), 12);

    store.set("precision", 3).unwrap();
    store.set("scientific", true).unwrap();
    store.reset("precision").unwrap();
    rikka_settings::flush_pending_writes();

    let (beside, _) = Store::load(schema(), layers).unwrap();
    assert!(beside.get::<bool>("scientific").unwrap());
    assert_eq!(beside.source("precision").unwrap(), Source::Default);
    assert_eq!(
        std::fs::read_to_string(user.path().join("com.example.calculator.toml")).unwrap(),
        "precision = 12\n",
        "the person's file is never written"
    );
}

#[test]
fn rejected_value_is_not_stored() {
    let (mut store, _system, user) = store();
    assert!(store.set("precision", 50).is_err());
    assert!(store.set("precision", "lots").is_err());
    assert!(store.set("angle-unit", "gradians").is_err());
    assert!(store.set("colour", "red").is_err());
    rikka_settings::flush_pending_writes();
    assert!(
        !user.path().join("com.example.calculator.toml").exists(),
        "a refusal writes nothing"
    );
}

#[test]
fn failed_write_is_reported_and_retried() {
    let (mut store, _system, _root, config) = store_that_cannot_write();

    store.set("precision", 12).unwrap();
    rikka_settings::flush_pending_writes();

    let error = store.set("precision", 12).unwrap_err();
    assert!(matches!(error, Error::WriteFile { .. }), "{error:?}");
    rikka_settings::flush_pending_writes();
    assert!(
        store.set("precision", 12).is_err(),
        "the file is still behind, and saying so once is not enough"
    );
    rikka_settings::flush_pending_writes();

    std::fs::remove_file(&config).unwrap();
    assert!(store.set("precision", 12).is_err(), "the last write's");
    rikka_settings::flush_pending_writes();

    let contents = std::fs::read_to_string(config.join("com.example.calculator.toml")).unwrap();
    assert!(contents.contains("precision = 12"), "{contents}");
    assert_eq!(store.get::<i64>("precision").unwrap(), 12);
    store.set("precision", 12).unwrap();
}

#[test]
fn invalid_entries_skip_only_themselves() {
    let (_, system, user) = store();
    put(
        &user,
        "precision = 99\nangle-unit = 'gradians'\nscientific = true\n",
    );
    let (store, report) = reload(&system, &user);

    assert!(store.get::<bool>("scientific").unwrap());
    assert_eq!(store.get::<i64>("precision").unwrap(), 10);
    assert_eq!(store.get::<String>("angle-unit").unwrap(), "degrees");
    assert_eq!(report.rejected.len(), 2);
}

#[test]
fn unparsable_file_skips_only_its_layer() {
    let (_, system, user) = store();
    put(&user, "precision = = 4\n");
    let (store, report) = reload(&system, &user);

    assert_eq!(store.get::<i64>("precision").unwrap(), 10);
    assert_eq!(report.rejected.len(), 1);
    assert!(report.rejected[0].key.is_none());
}

#[test]
fn unknown_setting_survives_write() {
    let (_, system, user) = store();
    put(&user, "precision = 12\ntheme-tint = 'warm'\n");
    let (mut store, report) = reload(&system, &user);
    assert!(report.is_empty(), "an unknown setting is not a complaint");
    assert_eq!(store.unknown().len(), 1);

    store.set("precision", 8).unwrap();
    rikka_settings::flush_pending_writes();

    let contents =
        std::fs::read_to_string(user.path().join("com.example.calculator.toml")).unwrap();
    assert!(contents.contains("theme-tint"), "{contents}");
    assert!(contents.contains("precision = 8"), "{contents}");
}
