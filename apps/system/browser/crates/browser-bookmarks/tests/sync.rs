use browser_bookmarks::{Error, LocalSide, Mirror, Plan};
use browser_store::{BookmarkSync, PinnedItem, PinnedPlace};

use crate::support::{Chrome, Desk, Shape, node, sync, url};

const START: &str = "A B | Home: x F(y) | Work: z";

fn desk() -> Desk {
    Desk::new(
        &["A", "B"],
        &[
            (
                "Home",
                vec![Shape::Pin("x"), Shape::Folder("F", vec![Shape::Pin("y")])],
            ),
            ("Work", vec![Shape::Pin("z")]),
        ],
    )
}

fn folder(desk: &Desk, name: &str) -> PinnedPlace {
    match desk.item(name) {
        PinnedItem::Folder(folder) => PinnedPlace::folder(folder),
        PinnedItem::Tab(_) => panic!("{name} is a pin"),
    }
}

fn assert_settled(chrome: &Chrome, desk: &mut Desk, synced: &mut Option<BookmarkSync>) {
    let before = chrome.changes();
    desk.changes = 0;
    sync(chrome, desk, synced);
    assert_eq!(chrome.changes(), before, "a second sync changes Chrome");
    assert_eq!(desk.changes, 0, "a second sync changes Rikka");
}

#[test]
fn first_sync_mirrors_rikka_under_other_bookmarks() {
    let chrome = Chrome::new();
    chrome.add("ブックマーク バー", "Bar", Some("https://bar.example/"));
    let mut desk = desk();
    let mut synced = None;

    sync(&chrome, &mut desk, &mut synced);

    assert_eq!(chrome.outline(), START);
    assert_eq!(desk.outline(), START);
    let tree = chrome.tree();
    assert_eq!(tree[0].children[0].children[0].title, "Bar");
    assert_settled(&chrome, &mut desk, &mut synced);
}

#[test]
fn first_sync_adopts_a_synced_rikka_folder_without_duplicates() {
    let chrome = Chrome::new();
    chrome.add("その他のブックマーク", "Rikka", None);
    chrome.add("Rikka", "Favorites", None);
    chrome.add("Favorites", "A", Some(&url("A")));
    chrome.add("Favorites", "C", Some(&url("C")));
    chrome.add("Rikka", "Spaces", None);
    chrome.add("Spaces", "Home", None);
    chrome.add("Home", "x", Some(&url("x")));
    chrome.add("Home", "w", Some(&url("w")));
    let mut desk = desk();
    let mut synced = None;

    sync(&chrome, &mut desk, &mut synced);

    let merged = "A C B | Home: x w F(y) | Work: z";
    assert_eq!(chrome.outline(), merged);
    assert_eq!(desk.outline(), merged);
    assert_settled(&chrome, &mut desk, &mut synced);
}

type Change = fn(&Chrome, &mut Desk);

#[test]
fn changes_on_either_side_meet_and_chrome_wins_conflicts() {
    let cases: [(&str, Change, &str, Option<&str>); 17] = [
        (
            "chrome rename wins over a local rename",
            |chrome, desk| {
                chrome.rename_titled("A", "A2");
                desk.rename_favorite("A", "A1");
            },
            "A2 B | Home: x F(y) | Work: z",
            None,
        ),
        (
            "local rename reaches chrome",
            |_, desk| desk.rename_favorite("B", "B1"),
            "A B1 | Home: x F(y) | Work: z",
            None,
        ),
        (
            "chrome removal removes the pin",
            |chrome, _| chrome.remove_titled("y"),
            "A B | Home: x F() | Work: z",
            None,
        ),
        (
            "local removal removes the bookmark",
            |_, desk| desk.remove_item("x"),
            "A B | Home: F(y) | Work: z",
            None,
        ),
        (
            "chrome edit brings back a locally removed pin",
            |chrome, desk| {
                desk.remove_item("z");
                chrome.rename_titled("z", "z2");
            },
            "A B | Home: x F(y) | Work: z2",
            None,
        ),
        (
            "local pin in a folder removed in chrome moves up",
            |chrome, desk| {
                chrome.remove_titled("F");
                let place = folder(desk, "F");
                desk.add_pin(place, &url("n"), "n");
            },
            "A B | Home: x n | Work: z",
            None,
        ),
        (
            "chrome order becomes the local order",
            |chrome, _| chrome.move_titled("B", "Favorites", 0),
            "B A | Home: x F(y) | Work: z",
            None,
        ),
        (
            "local move into a folder reaches chrome",
            |_, desk| {
                let place = folder(desk, "F").at(1);
                desk.move_item("x", place);
            },
            "A B | Home: F(y x) | Work: z",
            None,
        ),
        (
            "chrome move wins over a local move",
            |chrome, desk| {
                let place = folder(desk, "F");
                desk.move_item("x", place);
                chrome.move_titled("x", "Work", u32::MAX);
            },
            "A B | Home: F(y) | Work: z x",
            None,
        ),
        (
            "space folder added in chrome becomes a space",
            |chrome, _| {
                chrome.add("Spaces", "Play", None);
                chrome.add("Play", "p", Some(&url("p")));
            },
            "A B | Home: x F(y) | Work: z | Play: p",
            None,
        ),
        (
            "space renamed in chrome renames the space",
            |chrome, _| chrome.rename_titled("Work", "Job"),
            "A B | Home: x F(y) | Job: z",
            None,
        ),
        (
            "space added locally gets its folder",
            |_, desk| {
                desk.add_space("New");
            },
            "A B | Home: x F(y) | Work: z | New: ",
            None,
        ),
        (
            "local space removal removes its folder",
            |_, desk| desk.mirror.spaces.retain(|space| space.name != "Work"),
            "A B | Home: x F(y)",
            None,
        ),
        (
            "local folder removal removes its bookmarks",
            |_, desk| desk.remove_item("F"),
            "A B | Home: x | Work: z",
            None,
        ),
        (
            "space folder removed in chrome is uploaded again",
            |chrome, _| chrome.remove_titled("Work"),
            START,
            None,
        ),
        (
            "rikka folder removed in chrome is uploaded again",
            |chrome, _| chrome.remove_titled("Rikka"),
            START,
            None,
        ),
        (
            "folders inside favorites stay chrome's own",
            |chrome, _| {
                chrome.add("Favorites", "Saved", None);
                chrome.add("Saved", "s", Some(&url("s")));
                chrome.move_titled("Saved", "Favorites", 1);
            },
            START,
            Some("A Saved(s) B | Home: x F(y) | Work: z"),
        ),
    ];
    for (name, change, local, remote) in cases {
        let chrome = Chrome::new();
        let mut desk = desk();
        let mut synced = None;
        sync(&chrome, &mut desk, &mut synced);

        change(&chrome, &mut desk);
        sync(&chrome, &mut desk, &mut synced);

        assert_eq!(desk.outline(), local, "{name}: local");
        assert_eq!(chrome.outline(), remote.unwrap_or(local), "{name}: chrome");
        let before = chrome.changes();
        desk.changes = 0;
        sync(&chrome, &mut desk, &mut synced);
        assert_eq!(chrome.changes(), before, "{name}: settled in chrome");
        assert_eq!(desk.changes, 0, "{name}: settled locally");
    }
}

#[test]
fn crossed_folder_moves_keep_chrome_nesting() {
    let chrome = Chrome::new();
    let mut desk = Desk::new(
        &[],
        &[(
            "Home",
            vec![
                Shape::Folder("F", vec![Shape::Pin("y")]),
                Shape::Folder("G", vec![Shape::Pin("w")]),
            ],
        )],
    );
    let mut synced = None;
    sync(&chrome, &mut desk, &mut synced);

    chrome.move_titled("G", "F", u32::MAX);
    let place = folder(&desk, "G");
    desk.move_item("F", place);
    sync(&chrome, &mut desk, &mut synced);

    assert_eq!(desk.outline(), " | Home: F(y G(w))");
    assert_eq!(chrome.outline(), " | Home: F(y G(w))");
    assert_settled(&chrome, &mut desk, &mut synced);
}

#[test]
fn a_tree_without_other_bookmarks_is_refused() {
    let tree = [node(
        "0",
        "",
        None,
        vec![node("1", "Bar", None, Vec::new())],
    )];
    assert!(matches!(
        Plan::new(&tree, &Mirror::default(), None),
        Err(Error::NoOtherBookmarks)
    ));
}
