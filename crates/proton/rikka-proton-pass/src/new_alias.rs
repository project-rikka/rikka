use std::fmt;

use crate::new_item::NewItem;

#[must_use]
#[derive(Clone)]
pub struct NewAlias {
    pub(crate) prefix: String,
    pub(crate) signed_suffix: String,
    pub(crate) mailboxes: Vec<i64>,
    pub(crate) name: Option<String>,
    pub(crate) item: NewItem,
}

impl NewAlias {
    pub fn new(
        prefix: impl Into<String>,
        signed_suffix: impl Into<String>,
        item: NewItem,
    ) -> NewAlias {
        NewAlias {
            prefix: prefix.into(),
            signed_suffix: signed_suffix.into(),
            mailboxes: Vec::new(),
            name: None,
            item,
        }
    }

    pub fn with_mailbox(mut self, mailbox: i64) -> NewAlias {
        self.mailboxes.push(mailbox);
        self
    }

    pub fn with_mailboxes(mut self, mailboxes: impl IntoIterator<Item = i64>) -> NewAlias {
        self.mailboxes.extend(mailboxes);
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> NewAlias {
        self.name = Some(name.into());
        self
    }
}

impl fmt::Debug for NewAlias {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewAlias")
            .field("mailboxes", &self.mailboxes.len())
            .finish_non_exhaustive()
    }
}
