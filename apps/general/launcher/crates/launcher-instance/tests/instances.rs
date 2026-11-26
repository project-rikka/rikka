use launcher_instance::{Error, Instances};

#[test]
fn created_instances_get_unique_folders() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path().join("instances"));
    let first = instances.create("My Survival!").expect("first instance");
    let second = instances.create("My Survival!").expect("second instance");
    let odd = instances.create("../..").expect("odd name");
    assert_eq!(first.id, "my-survival");
    assert_eq!(second.id, "my-survival-2");
    assert_eq!(odd.id, "instance");
    assert_eq!(second.config.name, "My Survival!");

    let listing = instances.list().expect("instances list");
    assert_eq!(listing.instances.len(), 3);
    assert!(listing.problems.is_empty());

    instances.remove(&first.id).expect("instance is removed");
    assert!(matches!(
        instances.get(&first.id),
        Err(Error::NoInstance(_))
    ));
    assert!(matches!(
        instances.get("../instances"),
        Err(Error::NoInstance(_))
    ));
}

#[test]
fn damaged_config_is_reported_not_fatal() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    instances.create("Good").expect("instance");
    std::fs::create_dir(root.path().join("broken")).expect("folder");
    std::fs::write(root.path().join("broken/instance.json"), "{").expect("config");
    let listing = instances.list().expect("instances list");
    assert_eq!(listing.instances.len(), 1);
    assert_eq!(listing.problems.len(), 1);
}

#[test]
fn updates_merge_into_latest_config() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let created = instances.create("Timed").expect("instance");
    instances
        .update(&created.id, |config| {
            config.add_play_time(90, 1_700_000_000)
        })
        .expect("first update");
    instances
        .update(&created.id, |config| config.name = "Renamed".to_owned())
        .expect("second update");
    let updated = instances
        .update(&created.id, |config| {
            config.add_play_time(30, 1_700_000_500)
        })
        .expect("third update");
    assert_eq!(updated.config.name, "Renamed");
    let reloaded = instances.get(&created.id).expect("reloads");
    assert_eq!(reloaded.config.play_time, 120);
    assert_eq!(reloaded.config.last_played, Some(1_700_000_500));
    assert_eq!(reloaded.config.name, "Renamed");
}
