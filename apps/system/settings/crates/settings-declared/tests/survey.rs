use std::path::Path;

use rikka_settings::{Catalog, Key, Layers, Role, Schema, Scope, Source, Value};
use settings_declared::{SettingId, Survey};
use tempfile::TempDir;

fn catalog() -> Catalog {
    Catalog::new()
        .schema(
            Schema::new("rikka.windows")
                .key(Key::string("layout", "floating").choices(["floating", "tiling"]))
                .key(Key::int("gap", 8).range(0..=64))
                .key(Key::list("recent", []).role(Role::State)),
        )
        .instanced(
            "display",
            Schema::new("rikka.displays").key(Key::float("scale", 1.0).range(0.5..=4.0)),
        )
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
}

fn put(folder: &Path, domain: &str, contents: &str) {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(folder.join(format!("{domain}.toml")), contents).unwrap();
}

#[test]
fn changes_are_what_the_user_holds_apart_from_the_configuration() {
    let folders = Folders::new();
    put(
        folders.machine.path(),
        "rikka.windows",
        "layout = \"tiling\"\ngap = 4\n",
    );
    put(
        folders.user.path(),
        "rikka.windows",
        "layout = \"floating\"\ngap = 4\nrecent = [\"a\"]\n",
    );
    let survey = Survey::read(&catalog(), &folders.layers());
    assert!(survey.problems().is_empty(), "{:?}", survey.problems());

    let changed: Vec<&str> = survey
        .changed()
        .map(|setting| setting.id.name.as_str())
        .collect();
    assert_eq!(
        changed,
        ["layout"],
        "a value equal to the configuration is no change, and state is never offered"
    );
    let layout = survey.find(&id("rikka.windows", "layout")).unwrap();
    assert_eq!(layout.beneath, Value::String("tiling".to_owned()));
    assert_eq!(layout.beneath_source, Source::Declared(Scope::Machine));

    let declared: Vec<&str> = survey
        .declared()
        .map(|setting| setting.id.name.as_str())
        .collect();
    assert_eq!(declared, ["layout", "gap"]);
}

#[test]
fn locked_settings_are_declared_but_never_changed() {
    let folders = Folders::new();
    put(
        &folders.person.path().join("locked"),
        "rikka.windows",
        "gap = 16\n",
    );
    put(folders.user.path(), "rikka.windows", "gap = 2\n");
    let survey = Survey::read(&catalog(), &folders.layers());

    let gap = survey.find(&id("rikka.windows", "gap")).unwrap();
    assert!(gap.is_locked());
    assert!(gap.is_declared());
    assert!(!gap.is_changed());
    assert_eq!(gap.value, Value::Int(16));
}

#[test]
fn instanced_settings_come_from_the_instances_their_files_name() {
    let folders = Folders::new();
    put(
        folders.person.path(),
        "rikka.displays",
        "dell-u2720q-1a2b-scale = 1.5\n",
    );
    put(
        folders.user.path(),
        "rikka.displays",
        "dell-u2720q-1a2b-scale = 2.0\nlg-27gp850-scale = 1.25\n",
    );
    let survey = Survey::read(&catalog(), &folders.layers());

    let mut changed: Vec<(Option<&str>, &str)> = survey
        .changed()
        .map(|setting| (setting.instance.as_deref(), setting.key.as_str()))
        .collect();
    changed.sort_unstable();
    assert_eq!(
        changed,
        [
            (Some("dell-u2720q-1a2b"), "scale"),
            (Some("lg-27gp850"), "scale")
        ]
    );
    let dell = survey
        .find(&id("rikka.displays", "dell-u2720q-1a2b-scale"))
        .unwrap();
    assert_eq!(dell.beneath, Value::Float(1.5));
}

fn id(domain: &str, name: &str) -> SettingId {
    SettingId {
        domain: domain.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn invalid_declarations_are_problems() {
    let folders = Folders::new();
    put(folders.machine.path(), "rikka.windows", "gap = 500\n");
    let survey = Survey::read(&catalog(), &folders.layers());
    let problem = &survey.problems()[0];
    assert_eq!(problem.domain, "rikka.windows");
    assert_eq!(problem.key.as_deref(), Some("gap"));
}
