use std::ffi::OsString;
use std::path::PathBuf;

use rikka_instance::{LaunchRequest, Name};

fn args(values: &[&str]) -> Vec<OsString> {
    std::iter::once(OsString::from("program"))
        .chain(values.iter().map(OsString::from))
        .collect()
}

#[test]
fn app_id_maps_to_valid_bus_name() {
    for (id, bus, path) in [
        ("rikka-files", "org.rikka.Files", "/org/rikka/Files"),
        (
            "rikka-task-manager",
            "org.rikka.TaskManager",
            "/org/rikka/TaskManager",
        ),
        (
            "com.example.Thing",
            "com.example.Thing",
            "/com/example/Thing",
        ),
        ("my-app", "org.rikka.MyApp", "/org/rikka/MyApp"),
        ("7-zip", "org.rikka._7Zip", "/org/rikka/_7Zip"),
    ] {
        let name = Name::from_app_id(id).expect("a name");
        assert_eq!((name.bus(), name.path()), (bus, path), "{id}");
    }
    assert!(Name::from_app_id("").is_none());
    assert!(Name::from_app_id("--").is_none());
}

#[test]
fn forwards_uris_not_options() {
    let open = LaunchRequest::from_arguments(args(&[
        "--new-window",
        "https://example.com/a b",
        "/tmp/a b#c%d",
        "-",
        "--",
        "--not-an-option",
        "C:/models/figure.pmx",
    ]));

    assert!(open.has_flag("--new-window"));
    assert!(
        !open.has_flag("--not-an-option"),
        "it came after the options ended"
    );
    assert_eq!(open.uris().len(), 5);
    assert_eq!(open.uris()[0], "https://example.com/a b");
    assert_eq!(open.uris()[1], "file:///tmp/a%20b%23c%25d");
    assert!(open.uris()[3].ends_with("/--not-an-option"));
    assert!(
        open.uris()[4].starts_with("file://") && open.uris()[4].ends_with("/C:/models/figure.pmx"),
        "a drive letter is not a scheme"
    );
    assert_eq!(open.paths()[0], PathBuf::from("/tmp/a b#c%d"));
    assert!(LaunchRequest::from_arguments(args(&[])).is_bare());
    assert!(!LaunchRequest::from_arguments(args(&["--private-window"])).is_bare());

    assert!(
        LaunchRequest::from_uris(vec!["file://elsewhere/etc/hostname".to_owned()])
            .paths()
            .is_empty()
    );
}
