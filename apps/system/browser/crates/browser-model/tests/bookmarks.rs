use browser_bookmarks::Plan;
use browser_model::Node;
use browser_store::PinnedNode;
use rikka_web::{BookmarkId, BookmarkNode, FolderKind};

use crate::support::model;

fn folder(id: &str, title: &str, children: Vec<BookmarkNode>) -> BookmarkNode {
    BookmarkNode {
        id: BookmarkId::from_raw(id),
        parent_id: None,
        index: 0,
        title: title.to_owned(),
        url: None,
        folder_kind: None,
        date_added: None,
        children,
    }
}

fn page(id: &str, title: &str) -> BookmarkNode {
    BookmarkNode {
        url: Some(format!("https://{title}.example/")),
        ..folder(id, title, Vec::new())
    }
}

fn outline(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Folder(folder) => {
                format!("{}({})", folder.name, outline(&folder.children).join(" "))
            }
            Node::Pin(pin) => pin.saved.title.clone(),
        })
        .collect()
}

fn stored(nodes: &[PinnedNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node {
            PinnedNode::Folder { folder, children } => {
                format!("{}({})", folder.name, stored(children).join(" "))
            }
            PinnedNode::Tab(tab) => tab.title.clone().unwrap_or_default(),
        })
        .collect()
}

#[test]
fn chrome_bookmarks_land_in_the_model_and_the_store() {
    let mut model = model();
    let profile = model.profiles.list()[0].id;
    let tree = [folder(
        "0",
        "",
        vec![
            folder("1", "Bookmarks bar", Vec::new()),
            BookmarkNode {
                folder_kind: Some(FolderKind::Other),
                ..folder(
                    "2",
                    "Other bookmarks",
                    vec![folder(
                        "10",
                        "Rikka",
                        vec![
                            folder("11", "Favorites", vec![page("12", "c")]),
                            folder(
                                "13",
                                "Spaces",
                                vec![folder(
                                    "14",
                                    "Home",
                                    vec![folder("15", "G", vec![page("16", "p")]), page("17", "q")],
                                )],
                            ),
                        ],
                    )],
                )
            },
        ],
    )];

    let plan = Plan::new(&tree, &model.bookmark_mirror(profile), None).expect("plan");
    model.apply_bookmarks(profile, plan);

    let favorites: Vec<&str> = model
        .favorites
        .for_profile(profile)
        .iter()
        .map(|favorite| favorite.saved.url.as_str())
        .collect();
    assert_eq!(favorites, ["https://c.example/"]);
    let home = model
        .spaces
        .list()
        .iter()
        .find(|space| space.name == "Home")
        .expect("space from chrome")
        .id;
    assert_eq!(outline(model.pinned.tree(home)), ["G(p)", "q"]);
    model.store().flush().expect("store writes");
    let store = model.store();
    assert_eq!(store.favorites(profile).expect("favorites").len(), 1);
    assert_eq!(stored(&store.pinned(home).expect("pinned")), ["G(p)", "q"]);
    assert!(
        store
            .spaces()
            .expect("spaces")
            .iter()
            .any(|space| space.id == home)
    );
}
