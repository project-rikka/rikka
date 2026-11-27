use browser_store::BookmarkRoots;
use rikka_web::{BookmarkId, BookmarkNode, FolderKind};

use crate::error::Error;

pub const RIKKA: &str = "Rikka";
pub const FAVORITES: &str = "Favorites";
pub const SPACES: &str = "Spaces";

pub(crate) struct Layout<'a> {
    pub(crate) other: &'a BookmarkNode,
    pub(crate) rikka: Option<&'a BookmarkNode>,
    pub(crate) favorites: Option<&'a BookmarkNode>,
    pub(crate) spaces: Option<&'a BookmarkNode>,
}

impl<'a> Layout<'a> {
    pub(crate) fn locate(
        tree: &'a [BookmarkNode],
        roots: Option<&BookmarkRoots>,
    ) -> Result<Layout<'a>, Error> {
        let root = tree
            .iter()
            .find(|node| node.parent_id.is_none())
            .ok_or(Error::NoOtherBookmarks)?;
        let other = root
            .children
            .iter()
            .find(|node| node.folder_kind == Some(FolderKind::Other))
            .ok_or(Error::NoOtherBookmarks)?;
        let rikka = roots
            .and_then(|roots| find(root, &roots.rikka))
            .filter(|node| node.is_folder())
            .or_else(|| titled(other, RIKKA));
        let child = |stored: Option<&String>, title: &str| {
            rikka.and_then(|rikka| {
                stored
                    .and_then(|id| {
                        rikka
                            .children
                            .iter()
                            .find(|node| node.is_folder() && node.id.as_str() == id)
                    })
                    .or_else(|| titled(rikka, title))
            })
        };
        Ok(Layout {
            other,
            rikka,
            favorites: child(roots.map(|roots| &roots.favorites), FAVORITES),
            spaces: child(roots.map(|roots| &roots.spaces), SPACES),
        })
    }

    pub(crate) fn kept(node: Option<&BookmarkNode>, stored: Option<&String>) -> bool {
        node.zip(stored)
            .is_some_and(|(node, stored)| node.id.as_str() == stored)
    }

    pub(crate) fn id(node: Option<&BookmarkNode>) -> Option<BookmarkId> {
        node.map(|node| node.id.clone())
    }
}

fn find<'a>(node: &'a BookmarkNode, id: &str) -> Option<&'a BookmarkNode> {
    if node.id.as_str() == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, id))
}

fn titled<'a>(parent: &'a BookmarkNode, title: &str) -> Option<&'a BookmarkNode> {
    parent
        .children
        .iter()
        .find(|node| node.is_folder() && node.title == title)
}
