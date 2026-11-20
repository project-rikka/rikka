use std::os::unix::fs::PermissionsExt;

use futures_util::StreamExt;
use rikka_settings::seam::{Announcement, Settings, SettingsService};
use rikka_settings::{Key, Layers, Schema, Source};
use rikka_test_seam::Host;
use tempfile::TempDir;

fn appearance() -> Schema {
    Schema::new("rikka.appearance")
        .summary("How the desktop looks")
        .key(Key::string("theme", "rikka-dark"))
        .key(Key::float("corner-radius", 12.0).range(0.0..=32.0))
}

fn calculator() -> Schema {
    Schema::new("com.example.calculator")
        .key(Key::bool("scientific", false))
        .key(Key::int("precision", 10).range(0..=20))
}

fn desktop() -> (Host, Layers, TempDir, TempDir) {
    let system = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let layers = Layers::new(system.path(), user.path());
    let host = {
        let layers = layers.clone();
        Host::start(move |seam| {
            seam.register(
                SettingsService::with_layers(layers)
                    .with_schema(appearance())
                    .expect("session schema should be valid"),
            )
            .expect("rikka.settings should register");
        })
    };
    (host, layers, system, user)
}

#[tokio::test(flavor = "multi_thread")]
async fn write_through_session_reads_back() {
    let (host, layers, _system, user) = desktop();
    let mut settings =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers).unwrap();
    assert!(settings.is_shared());

    assert_eq!(settings.get::<i64>("precision").unwrap(), 10);
    assert_eq!(settings.source("precision").unwrap(), Source::Default);

    settings.set("precision", 12).unwrap();
    assert_eq!(settings.get::<i64>("precision").unwrap(), 12);
    assert_eq!(settings.source("precision").unwrap(), Source::User);

    rikka_settings::flush_pending_writes();
    let file = user.path().join("com.example.calculator.toml");
    let contents = std::fs::read_to_string(&file).unwrap();
    assert!(contents.contains("precision = 12"), "{contents}");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "settings may hold secrets, so only their owner reads them"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn session_rejects_invalid_value() {
    let (host, layers, _system, user) = desktop();
    let mut settings =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers).unwrap();

    assert!(settings.set("precision", 99).is_err());
    assert!(settings.set("precision", "lots").is_err());
    assert!(settings.set("colour", "red").is_err());

    rikka_settings::flush_pending_writes();
    assert!(!user.path().join("com.example.calculator.toml").exists());
    assert_eq!(settings.get::<i64>("precision").unwrap(), 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn write_is_announced_to_other_clients() {
    let (host, layers, ..) = desktop();
    let mut writer =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers.clone())
            .unwrap();
    let mut reader =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers).unwrap();
    let mut announcements = reader.changes().expect("shared store should announce");

    writer.set("scientific", true).unwrap();

    let announced = announcements.next().await.expect("session should announce");
    let Announcement::Changed(changed) = announced else {
        panic!("an announcement of what changed, and not a gap");
    };
    assert_eq!(changed.domain.as_str(), "com.example.calculator");
    assert!(changed.changes_key("scientific"));

    assert!(
        !reader.get::<bool>("scientific").unwrap(),
        "it is holding what it read before"
    );
    assert!(
        reader.apply_change(&changed),
        "the announcement is its own domain's"
    );
    assert!(reader.get::<bool>("scientific").unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn pending_write_visible_to_new_client() {
    let (host, layers, ..) = desktop();
    let mut writer =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers.clone())
            .unwrap();
    writer.set("precision", 12).unwrap();

    let mut reopened =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers).unwrap();
    assert_eq!(reopened.get::<i64>("precision").unwrap(), 12);

    reopened.set("scientific", true).unwrap();
    writer.refresh().unwrap();
    assert_eq!(writer.get::<i64>("precision").unwrap(), 12);
    assert!(writer.get::<bool>("scientific").unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn noop_write_is_not_announced() {
    let (host, layers, ..) = desktop();
    let mut settings =
        Settings::with_seam_and_layers(calculator(), host.client().clone(), layers).unwrap();
    let mut announcements = settings.changes().expect("shared store should announce");

    settings.set("precision", 10).unwrap();
    settings.set("precision", 11).unwrap();

    let announced = announcements
        .next()
        .await
        .expect("second write should change something");
    let Announcement::Changed(changed) = announced else {
        panic!("an announcement of what changed, and not a gap");
    };
    assert_eq!(changed.changes.len(), 1);
    assert!(changed.changes_key("precision"));
}

#[tokio::test(flavor = "multi_thread")]
async fn builtin_domains_cannot_be_redeclared() {
    let (host, layers, ..) = desktop();
    let hijacked = Schema::new("rikka.appearance").key(Key::int("theme", 0));

    let settings = Settings::with_seam_and_layers(hijacked, host.client().clone(), layers).unwrap();
    assert!(
        !settings.is_shared(),
        "a refused declaration falls back to the files rather than pretending"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn adopting_undeclared_domain_fails() {
    let (host, layers, ..) = desktop();
    let adopted =
        Settings::adopt_with_seam_and_layers("com.example.absent", host.client().clone(), layers);

    assert!(adopted.is_err());
}

#[test]
fn writes_files_without_session() {
    let system = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let mut settings =
        Settings::local_with_layers(calculator(), Layers::new(system.path(), user.path())).unwrap();

    assert!(!settings.is_shared());
    assert!(settings.changes().is_none());

    settings.set("precision", 3).unwrap();
    assert_eq!(settings.get::<i64>("precision").unwrap(), 3);

    rikka_settings::flush_pending_writes();
    assert!(user.path().join("com.example.calculator.toml").is_file());
}
