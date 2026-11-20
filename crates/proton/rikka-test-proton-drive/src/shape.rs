use crate::ids::{CONTENT, SPLIT};

const EVENTS_PAGE: usize = 50;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Corruption {
    #[default]
    None,
    Passphrase,
    Block,
    Manifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    OwnDraft,
    ForeignDraft,
}

#[derive(Debug, Clone)]
pub struct Shape {
    pub(crate) seeded: bool,
    pub(crate) corruption: Corruption,
    pub(crate) blocks: Vec<Vec<u8>>,
    pub(crate) extra_folders: usize,
    pub(crate) extra_files: Vec<String>,
    pub(crate) slow_blocks: bool,
    pub(crate) revision_history: bool,
    pub(crate) conflict: Option<Conflict>,
    pub(crate) failing_upload: bool,
    pub(crate) flaky_delete: bool,
    pub(crate) legacy_device: Option<String>,
    pub(crate) photo_library_missing: bool,
    pub(crate) member_share: bool,
    pub(crate) events_page: usize,
    pub(crate) convertible: Vec<String>,
}

impl Shape {
    #[must_use]
    pub fn empty() -> Shape {
        Shape {
            seeded: false,
            corruption: Corruption::None,
            blocks: Vec::new(),
            extra_folders: 0,
            extra_files: Vec::new(),
            slow_blocks: false,
            revision_history: false,
            conflict: None,
            failing_upload: false,
            flaky_delete: false,
            legacy_device: None,
            photo_library_missing: false,
            member_share: false,
            events_page: EVENTS_PAGE,
            convertible: Vec::new(),
        }
    }

    #[must_use]
    pub fn new() -> Shape {
        Shape {
            seeded: true,
            blocks: vec![CONTENT[..SPLIT].to_vec(), CONTENT[SPLIT..].to_vec()],
            ..Shape::empty()
        }
    }

    #[must_use]
    pub fn corrupted(mut self, corruption: Corruption) -> Shape {
        self.corruption = corruption;
        self
    }

    #[must_use]
    pub fn blocks(mut self, blocks: Vec<Vec<u8>>) -> Shape {
        self.blocks = blocks;
        self
    }

    #[must_use]
    pub fn extra_folders(mut self, count: usize) -> Shape {
        self.extra_folders = count;
        self
    }

    #[must_use]
    pub fn extra_files(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Shape {
        self.extra_files = names.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn slow_blocks(mut self) -> Shape {
        self.slow_blocks = true;
        self
    }

    #[must_use]
    pub fn revision_history(mut self) -> Shape {
        self.revision_history = true;
        self
    }

    #[must_use]
    pub fn conflict(mut self, conflict: Conflict) -> Shape {
        self.conflict = Some(conflict);
        self
    }

    #[must_use]
    pub fn failing_upload(mut self) -> Shape {
        self.failing_upload = true;
        self
    }

    #[must_use]
    pub fn flaky_delete(mut self) -> Shape {
        self.flaky_delete = true;
        self
    }

    #[must_use]
    pub fn legacy_device(mut self, name: impl Into<String>) -> Shape {
        self.legacy_device = Some(name.into());
        self
    }

    #[must_use]
    pub fn photo_library_missing(mut self) -> Shape {
        self.photo_library_missing = true;
        self
    }

    #[must_use]
    pub fn member_share(mut self) -> Shape {
        self.member_share = true;
        self
    }

    #[must_use]
    pub fn events_page(mut self, size: usize) -> Shape {
        self.events_page = size.max(1);
        self
    }

    #[must_use]
    pub fn convertible_invitation(mut self, link: impl Into<String>) -> Shape {
        self.convertible.push(link.into());
        self
    }
}

impl Default for Shape {
    fn default() -> Shape {
        Shape::new()
    }
}
