use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use desk_layout::WidgetId;

use crate::widgets::photos::album::Album;

#[derive(Default)]
pub(crate) struct Shelf {
    albums: RefCell<HashMap<Option<String>, Arc<Album>>>,
    shown: RefCell<HashMap<WidgetId, PathBuf>>,
}

impl Shelf {
    #[must_use]
    pub(crate) fn album(&self, folder: &Option<String>) -> Option<Arc<Album>> {
        self.albums.borrow().get(folder).cloned()
    }

    pub(crate) fn keep(&self, folder: &Option<String>, album: &Arc<Album>) {
        self.albums
            .borrow_mut()
            .insert(folder.clone(), Arc::clone(album));
    }

    #[must_use]
    pub(crate) fn shown(&self, widget: WidgetId) -> Option<PathBuf> {
        self.shown.borrow().get(&widget).cloned()
    }

    pub(crate) fn show(&self, widget: WidgetId, picture: PathBuf) {
        self.shown.borrow_mut().insert(widget, picture);
    }
}
