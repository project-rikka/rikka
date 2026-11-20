use proton_files::{Entry, Page, Tree};
use rikka_proton_drive::LinkId;

const ROOT: &str = "root";

fn entry(name: &str, folder: bool, size: i64, modified: i64) -> Entry {
    Entry {
        id: LinkId::from(name),
        name: name.to_owned(),
        folder,
        size,
        media_type: match folder {
            true => String::new(),
            false => "application/octet-stream".to_owned(),
        },
        modified: jiff::Timestamp::from_second(modified).unwrap(),
        trashed: false,
        pictured: false,
    }
}

fn root() -> LinkId {
    LinkId::from(ROOT)
}

fn cached(entries: Vec<Entry>) -> Tree {
    let mut tree = Tree::new();
    assert!(tree.load_cached_root(root(), ROOT.to_owned(), entries));
    tree
}

fn page(names: &[&str]) -> Page {
    Page {
        nodes: Vec::new(),
        entries: names.iter().map(|name| entry(name, false, 1, 1)).collect(),
    }
}

fn named(tree: &Tree) -> Vec<&str> {
    tree.entries()
        .iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

#[test]
fn empty_folder_fills_page_by_page() {
    let mut tree = cached(Vec::new());
    tree.begin(&root());

    assert!(tree.append_page(&root(), page(&["b.txt", "d.txt"])));
    assert_eq!(named(&tree), vec!["b.txt", "d.txt"]);

    assert!(tree.append_page(&root(), page(&["a.txt", "c.txt"])));
    assert_eq!(named(&tree), vec!["a.txt", "b.txt", "c.txt", "d.txt"]);

    assert!(tree.finish_listing(&root()));
    assert_eq!(named(&tree), vec!["a.txt", "b.txt", "c.txt", "d.txt"]);
}

#[test]
fn cached_entries_never_override_listing() {
    let mut tree = cached(Vec::new());
    tree.begin(&root());
    assert!(tree.append_page(&root(), page(&["fresh.txt"])));

    assert!(!tree.load_cached(&root(), vec![entry("stale.txt", false, 1, 1)]));
    assert_eq!(named(&tree), vec!["fresh.txt"]);
}

#[test]
fn ignores_page_of_replaced_listing() {
    let mut tree = cached(Vec::new());
    tree.begin(&LinkId::from("elsewhere"));

    assert!(!tree.append_page(&root(), page(&["late.txt"])));
    assert!(!tree.finish_listing(&root()));
    assert!(tree.entries().is_empty());
}
