#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Local,
    Remote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    name: String,
    kind: Kind,
    head: bool,
    checked_out_elsewhere: bool,
}

impl Branch {
    pub(crate) fn new(name: String, kind: Kind, head: bool, checked_out_elsewhere: bool) -> Branch {
        Branch {
            name,
            kind,
            head,
            checked_out_elsewhere,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub fn is_head(&self) -> bool {
        self.head
    }

    #[must_use]
    pub fn is_checked_out_elsewhere(&self) -> bool {
        self.checked_out_elsewhere
    }
}
