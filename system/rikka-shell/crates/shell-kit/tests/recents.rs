use shell_kit::{Recents, RecentsConfig, StateFile};

const CONFIG: RecentsConfig = RecentsConfig {
    file: "recents.json",
    field: "items",
    capacity: 3,
};

fn recorded(items: &[&str]) -> Recents<String> {
    let recents = Recents::read(CONFIG);
    for item in items {
        recents.record((*item).to_owned());
    }
    recents
}

#[test]
fn newest_comes_first_without_duplicates_within_capacity() {
    let recents = recorded(&["a", "b", "c", "b", "d"]);

    assert_eq!(recents.items(), ["d", "b", "c"]);
}

#[test]
fn recorded_items_survive_a_restart() {
    drop(recorded(&["a", "b", "a"]));

    assert_eq!(Recents::<String>::read(CONFIG).items(), ["a", "b"]);
}

#[test]
fn unreadable_file_is_set_aside_before_it_is_replaced() {
    let file = StateFile::state(CONFIG.file).expect("state directory");
    std::fs::write(file.path(), "{ not json").expect("write state file");

    let recents = Recents::<String>::read(CONFIG);
    assert!(recents.is_empty());
    recents.record("a".to_owned());
    drop(recents);

    let mut aside = file.path().as_os_str().to_owned();
    aside.push(".unreadable");
    assert_eq!(
        std::fs::read_to_string(aside).expect("set-aside copy"),
        "{ not json"
    );
    assert_eq!(Recents::<String>::read(CONFIG).items(), ["a"]);
}
