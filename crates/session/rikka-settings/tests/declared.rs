use std::path::Path;

use futures_util::StreamExt;
use rikka_settings::seam::{Announcement, Settings, SettingsService, protocol};
use rikka_settings::{Error, Key, Layers, Schema, Scope, Source, Store};
use rikka_test_seam::Host;
use tempfile::TempDir;

const DOMAIN: &str = "com.example.calculator";

fn schema() -> Schema {
    Schema::new(DOMAIN)
        .key(Key::bool("scientific", false))
        .key(Key::int("precision", 10).range(0..=20))
        .key(Key::string("angle-unit", "degrees").choices(["degrees", "radians"]))
}

struct Folders {
    system: TempDir,
    machine: TempDir,
    person: TempDir,
    user: TempDir,
}

impl Folders {
    fn new() -> Self {
        Self {
            system: tempfile::tempdir().unwrap(),
            machine: tempfile::tempdir().unwrap(),
            person: tempfile::tempdir().unwrap(),
            user: tempfile::tempdir().unwrap(),
        }
    }

    fn layers(&self) -> Layers {
        Layers::new(self.system.path(), self.user.path())
            .declared(Scope::Machine, self.machine.path())
            .declared(Scope::User, self.person.path())
    }

    fn store(&self) -> Store {
        let (store, report) = Store::load(schema(), self.layers()).unwrap();
        assert!(report.is_empty(), "{:?}", report.rejected);
        store
    }
}

fn put(folder: &Path, contents: &str) {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(folder.join(format!("{DOMAIN}.toml")), contents).unwrap();
}

#[test]
fn layers_resolve_from_default_to_locked() {
    let folders = Folders::new();
    put(folders.system.path(), "precision = 1\nscientific = true\n");
    put(folders.machine.path(), "precision = 2\n");
    put(
        folders.person.path(),
        "precision = 3\nangle-unit = \"radians\"\n",
    );
    put(folders.user.path(), "precision = 4\n");

    let store = folders.store();
    assert_eq!(store.get::<i64>("precision").unwrap(), 4);
    assert_eq!(store.source("precision").unwrap(), Source::User);
    assert_eq!(
        store.beneath_user("precision").unwrap().source,
        Source::Declared(Scope::User)
    );
    assert_eq!(
        store.source("angle-unit").unwrap(),
        Source::Declared(Scope::User)
    );
    assert_eq!(store.source("scientific").unwrap(), Source::System);

    put(&folders.machine.path().join("locked"), "precision = 7\n");
    let store = folders.store();
    assert_eq!(store.get::<i64>("precision").unwrap(), 7);
    assert_eq!(
        store.source("precision").unwrap(),
        Source::Locked(Scope::Machine)
    );

    put(&folders.person.path().join("locked"), "precision = 8\n");
    let store = folders.store();
    assert_eq!(store.get::<i64>("precision").unwrap(), 8);
    assert_eq!(
        store.source("precision").unwrap(),
        Source::Locked(Scope::User)
    );
}

#[test]
fn reset_reveals_the_declared_value() {
    let folders = Folders::new();
    put(folders.person.path(), "precision = 3\n");
    let mut store = folders.store();

    store.set("precision", 12).unwrap();
    assert_eq!(store.source("precision").unwrap(), Source::User);

    store.reset("precision").unwrap();
    assert_eq!(store.get::<i64>("precision").unwrap(), 3);
    assert_eq!(
        store.source("precision").unwrap(),
        Source::Declared(Scope::User)
    );
}

#[test]
fn locked_settings_refuse_writes_and_keep_the_users_value() {
    let folders = Folders::new();
    put(folders.user.path(), "precision = 4\n");
    put(&folders.machine.path().join("locked"), "precision = 7\n");
    let mut store = folders.store();

    assert!(store.is_locked("precision"));
    assert!(matches!(
        store.set("precision", 12),
        Err(Error::Locked { .. })
    ));
    assert!(matches!(
        store.reset("precision"),
        Err(Error::Locked { .. })
    ));
    assert_eq!(store.get::<i64>("precision").unwrap(), 7);
    assert_eq!(store.user_values().get("precision"), Some(&4.into()));

    store.set("scientific", true).unwrap();
    rikka_settings::flush_pending_writes();
    let written =
        std::fs::read_to_string(folders.user.path().join(format!("{DOMAIN}.toml"))).unwrap();
    assert!(written.contains("precision = 4"), "{written}");
}

#[test]
fn declared_files_may_name_settings_this_domain_lacks() {
    let folders = Folders::new();
    put(folders.machine.path(), "precision = 2\nretired = true\n");
    let (store, report) = Store::load(schema(), folders.layers()).unwrap();
    assert!(report.is_empty(), "{:?}", report.rejected);
    assert_eq!(store.get::<i64>("precision").unwrap(), 2);

    put(folders.machine.path(), "precision = 99\n");
    let (store, report) = Store::load(schema(), folders.layers()).unwrap();
    assert_eq!(
        report.rejected.len(),
        1,
        "an invalid declared value is reported"
    );
    assert_eq!(store.source("precision").unwrap(), Source::Default);
}

fn session(folders: &Folders) -> Host {
    let layers = folders.layers();
    Host::start(move |seam| {
        seam.register(SettingsService::with_layers(layers))
            .expect("rikka.settings should register");
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn reload_announces_a_changed_configuration() {
    let folders = Folders::new();
    let host = session(&folders);
    let mut settings =
        Settings::with_seam_and_layers(schema(), host.client().clone(), folders.layers()).unwrap();
    let mut changes = settings.changes().unwrap();

    put(folders.person.path(), "precision = 3\n");
    put(
        &folders.person.path().join("locked"),
        "angle-unit = \"radians\"\n",
    );
    let reloaded: Vec<String> = host
        .client()
        .call(protocol::reload())
        .await
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(reloaded, [DOMAIN]);

    let Some(Announcement::Changed(changed)) = changes.next().await else {
        panic!("a reload that moved settings is announced");
    };
    assert!(changed.reread);
    assert!(settings.apply_change(&changed));
    assert_eq!(settings.get::<i64>("precision").unwrap(), 3);
    assert!(settings.is_locked("angle-unit"));
    assert!(matches!(
        settings.set("angle-unit", "degrees"),
        Err(Error::Locked { .. })
    ));

    let reloaded: Vec<String> = host
        .client()
        .call(protocol::reload())
        .await
        .unwrap()
        .decode()
        .unwrap();
    assert!(
        reloaded.is_empty(),
        "an unchanged configuration moves nothing"
    );
}
