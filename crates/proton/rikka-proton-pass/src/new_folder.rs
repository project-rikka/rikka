use crate::identifier::FolderId;

#[must_use]
#[derive(Debug, Clone)]
pub struct NewFolder {
    pub(crate) name: String,
    pub(crate) parent: Option<FolderId>,
}

impl NewFolder {
    pub fn new(name: impl Into<String>) -> NewFolder {
        NewFolder {
            name: name.into(),
            parent: None,
        }
    }

    pub fn with_parent(mut self, parent: FolderId) -> NewFolder {
        self.parent = Some(parent);
        self
    }
}
