use std::collections::HashMap;

use rikka_thread::Attachment;
use rikka_ui::TextDocument;

#[derive(Debug, Default)]
pub struct Draft {
    pub text: TextDocument,
    pub attached: Vec<Attachment>,
}

impl Draft {
    #[must_use]
    pub fn new(text: TextDocument, attached: Vec<Attachment>) -> Self {
        Self { text, attached }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.attached.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Drafts {
    drafts: HashMap<String, Draft>,
}

impl Drafts {
    pub fn stash(&mut self, id: &str, draft: Draft) {
        if !draft.is_empty() {
            self.drafts.insert(id.to_owned(), draft);
        } else {
            self.drafts.remove(id);
        }
    }

    pub fn restore(&mut self, id: &str) -> Option<Draft> {
        self.drafts.remove(id)
    }

    pub fn forget(&mut self, id: &str) {
        self.drafts.remove(id);
    }
}
