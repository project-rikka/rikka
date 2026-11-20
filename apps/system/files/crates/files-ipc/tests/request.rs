use files_core::Location;
use files_ipc::Reveal;

fn uris(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn file_reveals_parent_folder() {
    let reveal = Reveal::classify(&uris(&["file:///etc/hostname"])).expect("valid uri");

    let (folder, selected) = reveal.destination().expect("destination");

    assert_eq!(folder, Location::from_path("/etc"));
    assert_eq!(selected, [Location::from_path("/etc/hostname")]);
}

#[test]
fn foreign_items_do_not_block_selection() {
    let reveal = Reveal::Items(vec![
        Location::from_path("/etc/hostname"),
        Location::from_path("/etc/hosts"),
        Location::from_path("/tmp/elsewhere"),
    ]);

    let (folder, selected) = reveal.destination().expect("destination");

    assert_eq!(folder, Location::from_path("/etc"));
    assert_eq!(selected.len(), 2);
}
