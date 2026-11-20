use crate::id::ItemId;

#[derive(Clone, Debug)]
pub(crate) enum Request {
    Activate { item: ItemId, at: (i32, i32) },
    Secondary { item: ItemId, at: (i32, i32) },
    Scroll { item: ItemId, delta: Scroll },
    ContextMenu { item: ItemId, at: (i32, i32) },
    Menu(ItemId),
    Choose { item: ItemId, row: i32 },
    Closed(ItemId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scroll {
    Vertical(i32),
    Horizontal(i32),
}

impl Scroll {
    #[must_use]
    pub(crate) fn parts(self) -> (i32, &'static str) {
        match self {
            Scroll::Vertical(delta) => (delta, "vertical"),
            Scroll::Horizontal(delta) => (delta, "horizontal"),
        }
    }
}

impl Request {
    #[must_use]
    pub(crate) fn item(&self) -> &ItemId {
        match self {
            Request::Activate { item, .. }
            | Request::Secondary { item, .. }
            | Request::Scroll { item, .. }
            | Request::ContextMenu { item, .. }
            | Request::Menu(item)
            | Request::Choose { item, .. }
            | Request::Closed(item) => item,
        }
    }
}
