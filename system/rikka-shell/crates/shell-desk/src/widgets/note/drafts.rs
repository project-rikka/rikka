use std::cell::RefCell;
use std::collections::HashMap;

use desk_layout::{Content, Layout, WidgetId};
use rikka_ui::TextDocument;

#[derive(Default)]
pub(crate) struct Drafts(RefCell<HashMap<WidgetId, Draft>>);

struct Draft {
    document: TextDocument,
    typed_on: Option<Option<String>>,
}

pub(crate) struct Unsaved {
    pub(crate) wallpaper: Option<String>,
    pub(crate) text: String,
}

impl Drafts {
    #[must_use]
    pub(crate) fn document(&self, id: WidgetId, note_text: &str) -> TextDocument {
        self.0
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| Draft {
                document: TextDocument::new(note_text),
                typed_on: None,
            })
            .document
            .clone()
    }

    pub(crate) fn typed(&self, id: WidgetId, wallpaper: Option<String>) {
        if let Some(draft) = self.0.borrow_mut().get_mut(&id) {
            draft.typed_on.get_or_insert(wallpaper);
        }
    }

    #[must_use]
    pub(crate) fn unkept(&self, id: WidgetId) -> Option<Unsaved> {
        let mut drafts = self.0.borrow_mut();
        let draft = drafts.get_mut(&id)?;
        let wallpaper = draft.typed_on.take()?;
        Some(Unsaved {
            wallpaper,
            text: draft.document.text(),
        })
    }

    pub(crate) fn settle(&self, layout: &Layout) {
        let drafts = self.0.borrow();
        for widget in &layout.widgets {
            let Content::Note(note) = &widget.content else {
                continue;
            };
            let Some(draft) = drafts
                .get(&widget.id)
                .filter(|draft| draft.typed_on.is_none())
            else {
                continue;
            };
            if draft.document.text() != note.text {
                draft.document.reset(&note.text);
            }
        }
    }
}
