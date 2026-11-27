use google_drive::{ROOT, Scope};
use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Root {
    MyDrive,
    SharedDrives,
    SharedWithMe,
    Recent,
    Starred,
    Trash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Crumb {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    Root(Root),
    Search(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Location {
    pub(crate) origin: Origin,
    pub(crate) trail: Vec<Crumb>,
}

impl Root {
    pub(crate) const ALL: [Root; 6] = [
        Root::MyDrive,
        Root::SharedDrives,
        Root::SharedWithMe,
        Root::Recent,
        Root::Starred,
        Root::Trash,
    ];

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Root::MyDrive => MaterialSymbol::HOME_STORAGE,
            Root::SharedDrives => MaterialSymbol::GROUPS,
            Root::SharedWithMe => MaterialSymbol::GROUP,
            Root::Recent => MaterialSymbol::SCHEDULE,
            Root::Starred => MaterialSymbol::STAR,
            Root::Trash => MaterialSymbol::DELETE,
        }
    }

    pub(crate) fn title(self) -> Label {
        match self {
            Root::MyDrive => tr!("google.drive.place.my-drive"),
            Root::SharedDrives => tr!("google.drive.place.shared-drives"),
            Root::SharedWithMe => tr!("google.drive.place.shared"),
            Root::Recent => tr!("google.drive.place.recent"),
            Root::Starred => tr!("google.drive.place.starred"),
            Root::Trash => tr!("google.drive.place.trash"),
        }
    }

    fn scope(self) -> Scope {
        match self {
            Root::MyDrive => Scope::my_drive(),
            Root::SharedDrives => Scope::SharedDrives,
            Root::SharedWithMe => Scope::SharedWithMe,
            Root::Recent => Scope::Recent,
            Root::Starred => Scope::Starred,
            Root::Trash => Scope::Trash,
        }
    }
}

impl Location {
    pub(crate) fn root(root: Root) -> Location {
        Location {
            origin: Origin::Root(root),
            trail: Vec::new(),
        }
    }

    pub(crate) fn search(text: &str) -> Location {
        Location {
            origin: Origin::Search(text.to_owned()),
            trail: Vec::new(),
        }
    }

    pub(crate) fn scope(&self) -> Scope {
        match (self.trail.last(), &self.origin) {
            (Some(crumb), _) => Scope::Folder(crumb.id.clone()),
            (None, Origin::Root(root)) => root.scope(),
            (None, Origin::Search(text)) => Scope::Search(text.clone()),
        }
    }

    pub(crate) fn folder(&self) -> Option<&str> {
        match (self.trail.last(), &self.origin) {
            (Some(crumb), _) => Some(&crumb.id),
            (None, Origin::Root(Root::MyDrive)) => Some(ROOT),
            (None, _) => None,
        }
    }

    pub(crate) fn is_trash(&self) -> bool {
        self.trail.is_empty() && self.origin == Origin::Root(Root::Trash)
    }

    pub(crate) fn entered(&self, crumb: Crumb) -> Location {
        let mut entered = self.clone();
        entered.trail.push(crumb);
        entered
    }

    pub(crate) fn ancestor(&self, depth: usize) -> Location {
        Location {
            origin: self.origin.clone(),
            trail: self.trail[..depth.min(self.trail.len())].to_vec(),
        }
    }

    pub(crate) fn parent(&self) -> Option<Location> {
        self.trail
            .len()
            .checked_sub(1)
            .map(|depth| self.ancestor(depth))
    }

    pub(crate) fn names(&self) -> Vec<Label> {
        let first = match &self.origin {
            Origin::Root(root) => root.title(),
            Origin::Search(text) => tr!("google.drive.search.results", query = text.as_str()),
        };
        std::iter::once(first)
            .chain(
                self.trail
                    .iter()
                    .map(|crumb| Label::from(crumb.name.clone())),
            )
            .collect()
    }
}
