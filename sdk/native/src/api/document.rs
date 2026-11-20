use rikka_ui::TextDocument;

pub(crate) fn text_document_new(text: &str) -> TextDocument {
    TextDocument::new(text)
}

pub(crate) fn text_document_text(document: &TextDocument) -> String {
    document.text()
}

pub(crate) fn text_document_set_text(document: &TextDocument, text: &str) {
    document.set_text(text);
}

pub(crate) fn text_document_reset(document: &TextDocument, text: &str) {
    document.reset(text);
}

pub(crate) fn text_document_length(document: &TextDocument) -> usize {
    document.len()
}

pub(crate) fn text_document_revision(document: &TextDocument) -> u64 {
    document.revision()
}

pub(crate) fn text_document_is_modified(document: &TextDocument) -> bool {
    document.is_modified()
}

pub(crate) fn text_document_mark_saved(document: &TextDocument) {
    document.mark_saved();
}

pub(crate) fn text_document_insert(document: &TextDocument, text: &str) {
    document.insert(text);
}

pub(crate) fn text_document_selected_text(document: &TextDocument) -> String {
    document.selected_text()
}

pub(crate) fn text_document_select_all(document: &TextDocument) {
    document.select_all();
}

pub(crate) fn text_document_can_undo(document: &TextDocument) -> bool {
    document.can_undo()
}

pub(crate) fn text_document_can_redo(document: &TextDocument) -> bool {
    document.can_redo()
}

pub(crate) fn text_document_undo(document: &TextDocument) -> bool {
    document.undo()
}

pub(crate) fn text_document_redo(document: &TextDocument) -> bool {
    document.redo()
}
