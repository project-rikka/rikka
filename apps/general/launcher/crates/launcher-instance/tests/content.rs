use launcher_instance::{ContentKind, Error, Instances, Place};

use crate::jar;

#[test]
fn content_lists_mods_with_metadata_and_state() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let mut instance = instances.create("Modded").expect("instance");
    let mods = instance.path.join("mods");
    std::fs::create_dir_all(&mods).expect("mods folder");
    jar(
        &mods.join("sodium.jar"),
        r#"{"id": "sodium", "name": "Sodium", "version": "0.8.0", "authors": ["JellySquid", {"name": "IMS"}]}"#,
    );
    jar(&mods.join("other.jar.disabled"), r#"{"id": "other"}"#);
    std::fs::write(mods.join("notes.txt"), "ignored").expect("stray file");

    let content = instance.content().expect("content lists");
    assert_eq!(content.len(), 2);
    let sodium = content
        .iter()
        .find(|file| file.file_name == "sodium.jar")
        .expect("sodium is listed");
    assert!(sodium.enabled);
    assert_eq!(sodium.kind, ContentKind::Mod);
    assert_eq!(sodium.title(), "Sodium");
    assert_eq!(sodium.key, "mods/sodium.jar");
    assert_eq!(sodium.record.sha512.len(), 128);
    let local = sodium.local.as_ref().expect("fabric metadata");
    assert_eq!(local.authors, ["JellySquid", "IMS"]);
    let other = content
        .iter()
        .find(|file| file.file_name == "other.jar")
        .expect("disabled mod is listed");
    assert!(!other.enabled);

    instance.set_enabled(sodium, false).expect("disables");
    assert!(mods.join("sodium.jar.disabled").is_file());
    let content = instance.content().expect("content lists again");
    let sodium = content
        .iter()
        .find(|file| file.file_name == "sodium.jar")
        .expect("sodium still listed");
    assert!(!sodium.enabled);
    instance.remove_content(sodium).expect("removes");
    assert!(!mods.join("sodium.jar.disabled").exists());
    assert!(!instance.config.content.contains_key("mods/sodium.jar"));
}

#[test]
fn data_packs_belong_to_worlds() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let mut instance = instances.create("Packs").expect("instance");
    let world = Place::World("New World".to_owned());
    let target = instance
        .target(ContentKind::DataPack, &world, "pack.zip")
        .expect("target in world");
    jar(&target, "{}");
    let content = instance.content().expect("content lists");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0].place, world);
    assert_eq!(content[0].key, "saves/New World/datapacks/pack.zip");
}

#[test]
fn unsafe_names_are_refused() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let instance = instances.create("Safe").expect("instance");
    for name in ["../escape.jar", "a/b.jar", ".hidden.jar", "c:\\x.jar", ""] {
        assert!(
            matches!(
                instance.target(ContentKind::Mod, &Place::Instance, name),
                Err(Error::FileName(_))
            ),
            "{name}"
        );
    }
    assert!(
        instance
            .target(
                ContentKind::DataPack,
                &Place::World("..".to_owned()),
                "x.zip"
            )
            .is_err()
    );
}

#[test]
fn import_refuses_duplicates() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let mut instance = instances.create("Imports").expect("instance");
    let source = root.path().join("lithium.jar");
    jar(&source, r#"{"id": "lithium"}"#);
    let key = instance
        .import(ContentKind::Mod, &Place::Instance, &source)
        .expect("imports");
    assert_eq!(key, "mods/lithium.jar");
    assert!(matches!(
        instance.import(ContentKind::Mod, &Place::Instance, &source),
        Err(Error::Exists(_))
    ));
}
