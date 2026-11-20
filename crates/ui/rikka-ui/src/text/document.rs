use std::cell::{Ref, RefCell};
use std::ops::Range;
use std::rc::Rc;

use crate::view::{Observed, Observers};
use rikka_text::{Affinity, ParagraphLayout, WordRule};
use rikka_text_document::{Content, Erase, LineEnding, Travel};

#[derive(Clone, Default)]
pub struct TextDocument(Rc<Shared>);

#[derive(Default)]
struct Shared {
    content: RefCell<Content>,
    observers: Observers,
}

impl TextDocument {
    pub fn new(text: impl AsRef<str>) -> Self {
        Self(Rc::new(Shared {
            content: RefCell::new(Content::new(text.as_ref())),
            observers: Observers::new(),
        }))
    }

    fn read(&self) -> Ref<'_, Content> {
        self.0.observers.read();
        self.0.content.borrow()
    }

    fn alter(&self, change: impl FnOnce(&mut Content) -> bool) -> bool {
        let changed = change(&mut self.0.content.borrow_mut());
        if changed {
            self.0.observers.reshaped();
        }
        changed
    }

    fn mark(&self, change: impl FnOnce(&mut Content) -> bool) {
        let changed = change(&mut self.0.content.borrow_mut());
        if changed {
            self.0.observers.changed();
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.read().text()
    }

    #[must_use]
    pub fn line(&self, index: usize) -> String {
        let mut line = String::new();
        self.read().line_into(index, &mut line);
        line
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.read().line_count()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.read().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    #[must_use]
    pub fn char_count(&self) -> usize {
        self.read().char_count()
    }

    #[must_use]
    pub fn line_ending(&self) -> LineEnding {
        self.read().line_ending()
    }

    pub fn set_line_ending(&self, ending: LineEnding) {
        self.alter(|content| content.set_line_ending(ending));
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.read().revision()
    }

    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.read().is_modified()
    }

    pub fn mark_saved(&self) {
        self.mark(Content::mark_saved);
    }

    #[must_use]
    pub fn caret(&self) -> usize {
        self.read().caret()
    }

    #[must_use]
    pub fn affinity(&self) -> Affinity {
        self.read().affinity()
    }

    #[must_use]
    pub fn anchor(&self) -> usize {
        self.read().anchor()
    }

    #[must_use]
    pub fn caret_line_column(&self) -> (usize, usize) {
        self.read().caret_line_column()
    }

    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        self.read().selection()
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.read().has_selection()
    }

    #[must_use]
    pub fn selected_text(&self) -> String {
        self.read().selected_text()
    }

    pub fn set_text(&self, text: impl AsRef<str>) {
        self.alter(|content| content.set_text(text.as_ref()));
    }

    pub fn reset(&self, text: impl AsRef<str>) {
        self.alter(|content| {
            content.reset(text.as_ref());
            true
        });
    }

    pub fn clear(&self) {
        self.set_text("");
    }

    pub fn replace(&self, range: Range<usize>, text: &str) -> bool {
        self.alter(|content| content.replace(range, text))
    }

    pub fn replace_each(&self, edits: &[(Range<usize>, String)]) -> bool {
        self.alter(|content| content.replace_each(edits))
    }

    pub fn append(&self, text: impl AsRef<str>) -> bool {
        self.alter(|content| content.append(text.as_ref()))
    }

    pub fn insert(&self, text: impl AsRef<str>) -> bool {
        self.alter(|content| content.insert_str(text.as_ref()))
    }

    pub fn insert_newline(&self, indented: bool) -> bool {
        self.alter(|content| content.insert_newline(indented))
    }

    pub fn interrupt(&self) {
        self.0.content.borrow_mut().interrupt();
    }

    pub fn backspace(&self) -> bool {
        self.alter(Content::backspace)
    }

    pub fn delete(&self) -> bool {
        self.alter(Content::delete)
    }

    pub fn erase(&self, erase: Erase, rule: WordRule) -> bool {
        self.alter(|content| content.erase(erase, rule))
    }

    pub fn travel(
        &self,
        to: Travel,
        extend: bool,
        line: Option<&ParagraphLayout>,
        rule: WordRule,
    ) -> bool {
        let moved = self.alter(|content| content.travel(to, extend, line, rule));
        self.interrupt();
        moved
    }

    pub fn delete_selection(&self) -> bool {
        self.alter(Content::delete_selection)
    }

    pub fn indent(&self, unit: &str) -> bool {
        self.alter(|content| content.indent(unit))
    }

    pub fn outdent(&self, unit: &str) -> bool {
        self.alter(|content| content.outdent(unit))
    }

    pub fn select_all(&self) -> bool {
        self.alter(Content::select_all)
    }

    pub fn select(&self, anchor: usize, caret: usize) {
        self.alter(|content| content.select(anchor, caret));
    }

    pub fn place_caret(&self, at: usize, extend: bool) -> bool {
        self.alter(|content| content.place_caret(at, extend))
    }

    pub fn place_cursor(&self, at: usize, affinity: Affinity, extend: bool) -> bool {
        self.alter(|content| content.place_cursor(at, affinity, extend))
    }

    pub fn place_cursor_unless_selected(&self, at: usize, affinity: Affinity) -> bool {
        if self.selection().contains(&at) {
            return false;
        }
        self.interrupt();
        self.place_cursor(at, affinity, false)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.read().can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.read().can_redo()
    }

    pub fn undo(&self) -> bool {
        self.alter(Content::undo)
    }

    pub fn redo(&self) -> bool {
        self.alter(Content::redo)
    }

    pub fn content(&self) -> Ref<'_, Content> {
        self.0.observers.painting();
        self.0.content.borrow()
    }

    #[must_use]
    pub fn same(&self, other: &TextDocument) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub fn edit(&self, edit: impl FnOnce(&mut Content) -> bool) -> bool {
        self.alter(edit)
    }
}

impl Observed for TextDocument {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl From<&str> for TextDocument {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for TextDocument {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl std::fmt::Debug for TextDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = self.0.content.borrow();
        f.debug_struct("TextDocument")
            .field("characters", &content.char_count())
            .field("lines", &content.line_count())
            .field("caret", &content.caret())
            .finish_non_exhaustive()
    }
}
