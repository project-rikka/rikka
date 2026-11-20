use browser_store::{FolderRemoval, PinnedNode, PinnedPlace, Store};

use crate::support;

fn outline(nodes: &[PinnedNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node {
            PinnedNode::Folder { folder, children } => {
                format!("{}[{}]", folder.name, outline(children).join(","))
            }
            PinnedNode::Tab(tab) => tab.title.clone().unwrap_or_else(|| tab.url.clone()),
        })
        .collect()
}

fn titled(store: &Store, place: PinnedPlace, title: &str) -> browser_store::PinnedTabId {
    store.pin_tab(
        place,
        format!("https://{title}.example/"),
        Some(title.to_owned()),
    )
}

#[test]
fn tree_keeps_folder_and_tab_order() {
    let store = Store::in_memory().unwrap();
    let (_, space) = support::space(&store, "Home");
    titled(&store, PinnedPlace::root(space), "a");
    let folder = store.create_pinned_folder(PinnedPlace::root(space), "F");
    titled(&store, PinnedPlace::folder(folder), "b");
    titled(&store, PinnedPlace::root(space).at(0), "c");
    titled(&store, PinnedPlace::folder(folder).at(0), "d");
    store.set_pinned_folder_expanded(folder, false);
    store.flush().unwrap();

    let tree = store.pinned(space).unwrap();
    assert_eq!(outline(&tree), ["c", "a", "F[d,b]"]);
    assert!(matches!(&tree[2], PinnedNode::Folder { folder, .. } if !folder.expanded));
}

#[test]
fn folder_removal_keeps_or_drops_children() {
    let store = Store::in_memory().unwrap();
    let (_, space) = support::space(&store, "Home");
    titled(&store, PinnedPlace::root(space), "first");
    let outer = store.create_pinned_folder(PinnedPlace::root(space), "Kept");
    titled(&store, PinnedPlace::folder(outer), "x");
    let nested = store.create_pinned_folder(PinnedPlace::folder(outer), "Nested");
    titled(&store, PinnedPlace::folder(nested), "y");
    titled(&store, PinnedPlace::root(space), "last");

    store.remove_pinned_folder(outer, FolderRemoval::KeepContents);
    store.flush().unwrap();
    assert_eq!(
        outline(&store.pinned(space).unwrap()),
        ["first", "x", "Nested[y]", "last"]
    );

    store.remove_pinned_folder(nested, FolderRemoval::WithContents);
    store.flush().unwrap();
    assert_eq!(
        outline(&store.pinned(space).unwrap()),
        ["first", "x", "last"]
    );
}
