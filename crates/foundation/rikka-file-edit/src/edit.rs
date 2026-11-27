use crate::anchor::Anchor;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Replace {
        first: Anchor,
        last: Anchor,
        text: String,
    },
    InsertAfter {
        anchor: Anchor,
        text: String,
    },
    InsertBefore {
        anchor: Anchor,
        text: String,
    },
}

impl Edit {
    pub(crate) fn text(&self) -> &str {
        match self {
            Edit::Replace { text, .. }
            | Edit::InsertAfter { text, .. }
            | Edit::InsertBefore { text, .. } => text,
        }
    }

    pub(crate) fn anchors(&self) -> Vec<Anchor> {
        match self {
            Edit::Replace { first, last, .. } if first == last => vec![*first],
            Edit::Replace { first, last, .. } => vec![*first, *last],
            Edit::InsertAfter { anchor, .. } | Edit::InsertBefore { anchor, .. } => vec![*anchor],
        }
    }
}
