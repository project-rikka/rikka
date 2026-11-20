use rikka_clipboard_core::Selection;

#[derive(Clone, Debug)]
pub struct Change {
    selection: Selection,
    ours: bool,
    mime_types: Vec<String>,
}

impl Change {
    pub(crate) fn new(selection: Selection, ours: bool, mime_types: Vec<String>) -> Change {
        Change {
            selection,
            ours,
            mime_types,
        }
    }

    #[must_use]
    pub fn selection(&self) -> Selection {
        self.selection
    }

    #[must_use]
    pub fn is_ours(&self) -> bool {
        self.ours
    }

    #[must_use]
    pub fn mime_types(&self) -> &[String] {
        &self.mime_types
    }

    #[must_use]
    pub fn offers(&self, mime: &str) -> bool {
        self.mime_types.iter().any(|known| known == mime)
    }
}

pub(crate) type Watcher = Box<dyn Fn(&Change) + Send>;
