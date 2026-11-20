use rikka_ui::TextDocument;

#[derive(Clone, Debug)]
pub struct Amendment {
    stamp: u64,
    draft: TextDocument,
    paths: Option<Vec<String>>,
    restore: bool,
}

impl Amendment {
    pub(crate) fn new(stamp: u64, text: &str) -> Amendment {
        Amendment {
            stamp,
            draft: TextDocument::new(text),
            paths: None,
            restore: true,
        }
    }

    #[must_use]
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.draft.text()
    }

    #[must_use]
    pub fn restores(&self) -> bool {
        self.restore && !self.paths().is_empty()
    }

    #[must_use]
    pub fn paths(&self) -> &[String] {
        self.paths.as_deref().unwrap_or_default()
    }

    pub(crate) fn draft(&self) -> &TextDocument {
        &self.draft
    }

    pub(crate) fn restoring(&self) -> bool {
        self.restore
    }

    pub(crate) fn touching(&mut self, paths: Vec<String>) {
        self.paths = Some(paths);
    }

    pub(crate) fn toggle_restore(&mut self) {
        self.restore = !self.restore;
    }
}

impl PartialEq for Amendment {
    fn eq(&self, other: &Amendment) -> bool {
        self.stamp == other.stamp && self.paths == other.paths && self.restore == other.restore
    }
}
