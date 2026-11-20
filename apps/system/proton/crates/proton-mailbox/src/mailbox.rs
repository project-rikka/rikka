use std::collections::HashMap;
use std::sync::Arc;

use rikka_proton_mail::{Event, LabelId, SystemLabel};

use crate::ask::Ask;
use crate::folder::{Folder, Kind};
use crate::ops::Listing;
use crate::row::{Row, RowId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Invalidation {
    pub folders: bool,
    pub listing: bool,
    pub counts: bool,
}

impl Invalidation {
    #[must_use]
    pub fn any(self) -> bool {
        self.folders || self.listing || self.counts
    }
}

#[derive(Debug)]
pub struct Mailbox {
    folders: Vec<Folder>,
    shown: LabelId,
    rows: Arc<Vec<Row>>,
    plain: bool,
    listed: bool,
    folder_rows: HashMap<LabelId, Arc<Vec<Row>>>,
    page: u32,
    more: bool,
}

impl Mailbox {
    #[must_use]
    pub fn new() -> Mailbox {
        Mailbox {
            folders: Vec::new(),
            shown: SystemLabel::Inbox.id(),
            rows: Arc::new(Vec::new()),
            plain: true,
            listed: false,
            folder_rows: HashMap::new(),
            page: 0,
            more: false,
        }
    }

    #[must_use]
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }

    #[must_use]
    pub fn folder(&self, id: &LabelId) -> Option<&Folder> {
        self.folders.iter().find(|folder| &folder.id == id)
    }

    #[must_use]
    pub fn shown(&self) -> &LabelId {
        &self.shown
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.folder(&self.shown)
            .map_or(Kind::Inbox, |folder| folder.kind)
    }

    pub fn show(&mut self, folder: LabelId) {
        if self.shown == folder {
            return;
        }
        if self.plain && !self.rows.is_empty() {
            self.folder_rows
                .insert(self.shown.clone(), Arc::clone(&self.rows));
        }
        self.rows = self.folder_rows.get(&folder).cloned().unwrap_or_default();
        self.shown = folder;
        self.plain = true;
        self.listed = false;
        self.page = 0;
        self.more = !self.rows.is_empty();
    }

    pub fn clear_folder_cache(&mut self) {
        self.folder_rows.clear();
    }

    #[must_use]
    pub fn rows(&self) -> &Arc<Vec<Row>> {
        &self.rows
    }

    #[must_use]
    pub fn row(&self, at: usize) -> Option<&Row> {
        self.rows.get(at)
    }

    #[must_use]
    pub fn place(&self, id: &RowId) -> Option<usize> {
        self.rows.iter().position(|row| &row.id == id)
    }

    #[must_use]
    pub fn page(&self) -> u32 {
        self.page
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn set_folders(&mut self, folders: Vec<Folder>) {
        self.folders = folders;
        if !self.folders.iter().any(|folder| folder.id == self.shown) {
            self.shown = SystemLabel::Inbox.id();
        }
    }

    pub fn apply_listing(&mut self, listing: Listing, ask: &Ask) {
        self.more = listing.more;
        self.page = ask.page;
        self.plain = ask.keyword.is_none() && !ask.unread;
        self.listed = true;
        match ask.is_first() {
            true => self.rows = Arc::new(listing.rows),
            false => {
                let rows = Arc::make_mut(&mut self.rows);
                for row in listing.rows {
                    if !rows.iter().any(|existing| existing.id == row.id) {
                        rows.push(row);
                    }
                }
            }
        }
    }

    pub fn load_cached(&mut self, folder: &LabelId, rows: Vec<Row>) -> bool {
        if &self.shown != folder || self.listed || !self.rows.is_empty() {
            return false;
        }
        self.rows = Arc::new(rows);
        self.plain = true;
        self.page = 0;
        self.more = true;
        true
    }

    pub fn set_read(&mut self, ids: &[RowId], read: bool) {
        for row in self.each(ids) {
            row.unread = !read;
        }
    }

    pub fn set_starred(&mut self, ids: &[RowId], starred: bool) {
        let star = SystemLabel::Starred.id();
        for row in self.each(ids) {
            row.starred = starred;
            row.labels.retain(|label| label != &star);
            if starred {
                row.labels.push(star.clone());
            }
        }
    }

    pub fn set_label(&mut self, ids: &[RowId], label: &LabelId, on: bool) {
        for row in self.each(ids) {
            row.labels.retain(|existing| existing != label);
            if on {
                row.labels.push(label.clone());
            }
        }
    }

    pub fn remove(&mut self, ids: &[RowId]) {
        Arc::make_mut(&mut self.rows).retain(|row| !ids.contains(&row.id));
    }

    fn each<'a>(&'a mut self, ids: &'a [RowId]) -> impl Iterator<Item = &'a mut Row> {
        Arc::make_mut(&mut self.rows)
            .iter_mut()
            .filter(|row| ids.contains(&row.id))
    }

    pub fn apply_event(&mut self, event: &Event, grouped: bool) -> Invalidation {
        if event.needs_refresh() {
            return Invalidation {
                folders: true,
                listing: true,
                counts: false,
            };
        }

        let listing = !event.messages().is_empty() || !event.conversations().is_empty();
        let counts = match grouped {
            true => event.conversation_counts(),
            false => event.message_counts(),
        };
        let counted = counts.is_some();
        for count in counts.unwrap_or_default() {
            if let Some(folder) = self
                .folders
                .iter_mut()
                .find(|folder| &folder.id == count.label_id())
            {
                folder.unread = count.unread();
                folder.total = count.total();
            }
        }
        Invalidation {
            folders: !event.labels().is_empty() || (listing && !counted),
            listing,
            counts: counted,
        }
    }
}

impl Default for Mailbox {
    fn default() -> Mailbox {
        Mailbox::new()
    }
}
