use rikka_ui::text::{Content, Endpoints, SelectionOrigin};

#[derive(Debug)]
pub(crate) struct Gesture {
    origin: SelectionOrigin,
    content: u64,
    revision: u64,
    reached: Endpoints,
}

impl Gesture {
    pub(crate) fn record(origin: SelectionOrigin, content: &Content) -> Self {
        Self {
            origin,
            content: content.id(),
            revision: content.revision(),
            reached: endpoints_of(content),
        }
    }

    pub(crate) fn origin(&self) -> SelectionOrigin {
        self.origin
    }

    pub(crate) fn holds(&self, content: &Content) -> bool {
        self.content == content.id()
            && self.revision == content.revision()
            && self.reached == endpoints_of(content)
    }
}

fn endpoints_of(content: &Content) -> Endpoints {
    Endpoints {
        anchor: content.anchor(),
        focus: content.caret(),
    }
}
