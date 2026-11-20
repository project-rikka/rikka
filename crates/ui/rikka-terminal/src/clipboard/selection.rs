use rikka_ui::Selection;

pub(crate) fn write(selection: Selection, text: &str) {
    rikka_ui::clipboard::set_text_of(selection, text);
}

pub(crate) fn away(work: impl FnOnce() + Send + 'static) {
    let started = std::thread::Builder::new()
        .name(String::from("rikka-terminal-clipboard"))
        .spawn(work);
    if let Err(error) = started {
        tracing::warn!("cannot open clipboard: {error}");
    }
}
