use std::cell::RefCell;
use std::rc::Rc;

use paint_document::FigureId;
use rikka_app::prelude::{Color, TextDocument};
use rikka_figure::{Lettering, Typeset};
use rikka_figure_editing::text_with_preedit;
use rikka_ui::{Observed, Observers};

use crate::options::Letters;

#[derive(Clone, Default)]
pub(crate) struct Typing(Rc<Inner>);

#[derive(Default)]
struct Inner {
    draft: RefCell<Option<Draft>>,
    settling: RefCell<Option<(Lettering, u64)>>,
    typeset: RefCell<Option<(Lettering, Typeset)>>,
    observers: Observers,
}

#[derive(Clone)]
pub(crate) struct Draft {
    pub(crate) document: TextDocument,
    pub(crate) origin: (i32, i32),
    pub(crate) editing: Option<FigureId>,
}

impl Draft {
    pub(crate) fn lettering(&self, letters: &Letters, color: Color) -> Lettering {
        letters.lettering(self.document.text(), self.origin, color)
    }

    pub(crate) fn display_lettering(&self, letters: &Letters, color: Color) -> Lettering {
        letters.lettering(text_with_preedit(&self.document), self.origin, color)
    }
}

impl Typing {
    pub(crate) fn same(&self, other: &Typing) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn begin(&self, origin: (i32, i32)) {
        *self.0.draft.borrow_mut() = Some(Draft {
            document: TextDocument::new(""),
            origin,
            editing: None,
        });
        self.0.observers.changed();
    }

    pub(crate) fn edit(&self, figure: FigureId, lettering: &Lettering) {
        let document = TextDocument::new(&lettering.text);
        document.edit(|content| content.place_caret(content.len(), false));
        *self.0.draft.borrow_mut() = Some(Draft {
            document,
            origin: lettering.origin,
            editing: Some(figure),
        });
        self.0.observers.changed();
    }

    pub(crate) fn draft(&self) -> Option<Draft> {
        self.0.observers.read();
        self.0.draft.borrow().clone()
    }

    pub(crate) fn place(&self, origin: (i32, i32)) {
        let moved = match self.0.draft.borrow_mut().as_mut() {
            Some(draft) if draft.origin != origin => {
                draft.origin = origin;
                true
            }
            _ => false,
        };
        if moved {
            self.0.observers.changed();
        }
    }

    pub(crate) fn take(&self) -> Option<Draft> {
        let draft = self.0.draft.borrow_mut().take();
        if draft.is_some() {
            self.0.observers.changed();
        }
        draft
    }

    pub(crate) fn settle(&self, lettering: Lettering, generation: u64) {
        *self.0.settling.borrow_mut() = Some((lettering, generation));
        self.0.observers.changed();
    }

    pub(crate) fn settled(&self, generation: u64) {
        let done = self
            .0
            .settling
            .borrow()
            .as_ref()
            .is_some_and(|(_, settles)| *settles <= generation);
        if done {
            *self.0.settling.borrow_mut() = None;
            self.0.observers.changed();
        }
    }

    pub(crate) fn settling(&self) -> Option<Lettering> {
        self.0.observers.read();
        self.0
            .settling
            .borrow()
            .as_ref()
            .map(|(lettering, _)| lettering.clone())
    }

    pub(crate) fn typeset(&self, lettering: Lettering) -> Typeset {
        let mut cache = self.0.typeset.borrow_mut();
        if let Some((shaped, typeset)) = cache.as_ref()
            && *shaped == lettering
        {
            return typeset.clone();
        }
        let typeset = Typeset::new(&lettering);
        *cache = Some((lettering, typeset.clone()));
        typeset
    }
}

impl Observed for Typing {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for Typing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typing")
            .field("typing", &self.0.draft.borrow().is_some())
            .field("settling", &self.0.settling.borrow().is_some())
            .finish()
    }
}
