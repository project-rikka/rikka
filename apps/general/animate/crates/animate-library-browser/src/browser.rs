use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use animate_library::{Catalog, Details, Entry, EntryId, Shelf, ThumbnailSize};
use rikka_picture::Feed;
use rikka_ui::TextDocument;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    Menu,
}

#[derive(Debug)]
struct Previewing {
    entry: EntryId,
    feed: Feed,
}

#[derive(Debug)]
pub struct Browser {
    shelf: Shelf,
    query: TextDocument,
    terms: Vec<String>,
    folder: Option<Arc<str>>,
    size: ThumbnailSize,
    popup: Option<Popup>,
    previewing: Option<Previewing>,
    lifted: Option<Arc<Entry>>,
    selected: Option<EntryId>,
    listed: RefCell<Option<Listed>>,
}

#[derive(Debug)]
struct Listed {
    catalog: Arc<Catalog>,
    shelf: Shelf,
    folder: Option<Arc<str>>,
    terms: Vec<String>,
    entries: Rc<[Arc<Entry>]>,
}

impl Browser {
    #[must_use]
    pub fn new(shelf: Shelf, size: ThumbnailSize) -> Self {
        Self {
            shelf,
            query: TextDocument::new(""),
            terms: Vec::new(),
            folder: None,
            size,
            popup: None,
            previewing: None,
            lifted: None,
            selected: None,
            listed: RefCell::new(None),
        }
    }

    #[must_use]
    pub fn shelf(&self) -> Shelf {
        self.shelf
    }

    pub fn choose_shelf(&mut self, shelf: Shelf) {
        if self.shelf != shelf {
            self.shelf = shelf;
            self.folder = None;
            self.selected = None;
        }
    }

    #[must_use]
    pub fn query(&self) -> &TextDocument {
        &self.query
    }

    pub fn typed(&mut self) {
        self.terms = self
            .query
            .text()
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
    }

    #[must_use]
    pub fn folder(&self) -> Option<&Arc<str>> {
        self.folder.as_ref()
    }

    pub fn choose_folder(&mut self, folder: Option<Arc<str>>) {
        self.folder = folder;
    }

    #[must_use]
    pub fn size(&self) -> ThumbnailSize {
        self.size
    }

    pub fn set_size(&mut self, size: ThumbnailSize) {
        self.size = size;
    }

    #[must_use]
    pub fn popup(&self) -> Option<Popup> {
        self.popup
    }

    pub fn toggle_popup(&mut self, popup: Popup) {
        self.popup = match self.popup == Some(popup) {
            true => None,
            false => Some(popup),
        };
    }

    pub fn close_popup(&mut self) {
        self.popup = None;
    }

    #[must_use]
    pub fn preview_of(&self, entry: EntryId) -> Option<&Feed> {
        self.previewing
            .as_ref()
            .filter(|previewing| previewing.entry == entry)
            .map(|previewing| &previewing.feed)
    }

    #[must_use]
    pub fn previewed(&self) -> Option<EntryId> {
        self.previewing.as_ref().map(|previewing| previewing.entry)
    }

    pub fn show_preview(&mut self, entry: EntryId, feed: Feed) {
        self.previewing = Some(Previewing { entry, feed });
    }

    pub fn end_preview(&mut self) {
        self.previewing = None;
    }

    #[must_use]
    pub fn selected(&self) -> Option<EntryId> {
        self.selected
    }

    pub fn select(&mut self, entry: EntryId) {
        self.selected = Some(entry);
    }

    #[must_use]
    pub fn lifted(&self) -> Option<&Arc<Entry>> {
        self.lifted.as_ref()
    }

    pub fn lift(&mut self, entry: Arc<Entry>) {
        self.lifted = Some(entry);
    }

    pub fn take_lifted(&mut self) -> Option<Arc<Entry>> {
        self.lifted.take()
    }

    #[must_use]
    pub fn shown(&self, catalog: &Arc<Catalog>) -> Rc<[Arc<Entry>]> {
        let mut listed = self.listed.borrow_mut();
        if let Some(listed) = listed.as_ref()
            && Arc::ptr_eq(&listed.catalog, catalog)
            && listed.shelf == self.shelf
            && listed.folder == self.folder
            && listed.terms == self.terms
        {
            return Rc::clone(&listed.entries);
        }
        let shelf = catalog.shelf(self.shelf);
        let matches = |entry: &&Arc<Entry>| self.matches(entry);
        let entries: Rc<[Arc<Entry>]> = match &self.folder {
            Some(folder) => shelf.within(folder).filter(matches).cloned().collect(),
            None => shelf.entries().iter().filter(matches).cloned().collect(),
        };
        *listed = Some(Listed {
            catalog: Arc::clone(catalog),
            shelf: self.shelf,
            folder: self.folder.clone(),
            terms: self.terms.clone(),
            entries: Rc::clone(&entries),
        });
        entries
    }

    fn matches(&self, entry: &Entry) -> bool {
        if self.terms.is_empty() {
            return true;
        }
        let mut haystack = format!("{} {}", entry.name(), entry.folder());
        match entry.details() {
            Details::Model(model) => {
                for part in [&model.title, &model.author].into_iter().flatten() {
                    haystack.push(' ');
                    haystack.push_str(part);
                }
            }
            Details::Motion(motion) => {
                if let Some(target) = &motion.target {
                    haystack.push(' ');
                    haystack.push_str(target);
                }
            }
            Details::Sound(sound) => {
                for part in [&sound.title, &sound.artist, &sound.album]
                    .into_iter()
                    .flatten()
                {
                    haystack.push(' ');
                    haystack.push_str(part);
                }
            }
            Details::Pending | Details::Unreadable(_) => {}
        }
        let haystack = haystack.to_lowercase();
        self.terms.iter().all(|term| haystack.contains(term))
    }
}
