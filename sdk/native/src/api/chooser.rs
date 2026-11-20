use std::path::PathBuf;

use rikka_dialog::{Chooser, Filter};

use crate::abi::violation;
use crate::ffi;

#[derive(Debug)]
pub(crate) struct FileChooser {
    chooser: Chooser,
    filters: Vec<Filter>,
}

impl FileChooser {
    fn update(&mut self, change: impl FnOnce(Chooser) -> Chooser) {
        self.chooser = change(std::mem::take(&mut self.chooser));
    }

    fn update_filter(&mut self, change: impl FnOnce(Filter) -> Filter) {
        let last = self
            .filters
            .pop()
            .unwrap_or_else(|| violation("no filter was added"));
        self.filters.push(change(last));
    }

    pub(crate) fn into_chooser(self) -> Chooser {
        self.filters.into_iter().fold(self.chooser, Chooser::filter)
    }
}

pub(crate) fn file_chooser_new(purpose: ffi::FileChooserPurpose, title: &str) -> FileChooser {
    let chooser = match purpose {
        ffi::FileChooserPurpose::Open => Chooser::open(title),
        ffi::FileChooserPurpose::Save => Chooser::save(title),
    };
    FileChooser {
        chooser,
        filters: Vec::new(),
    }
}

pub(crate) fn file_chooser_set_accept_label(chooser: &mut FileChooser, label: &str) {
    chooser.update(|inner| inner.accept_label(label));
}

pub(crate) fn file_chooser_set_multiple(chooser: &mut FileChooser, multiple: bool) {
    chooser.update(|inner| inner.multiple(multiple));
}

pub(crate) fn file_chooser_set_directory(chooser: &mut FileChooser, directory: bool) {
    chooser.update(|inner| inner.directory(directory));
}

pub(crate) fn file_chooser_set_folder(chooser: &mut FileChooser, path: &str) {
    chooser.update(|inner| inner.current_folder(PathBuf::from(path)));
}

pub(crate) fn file_chooser_set_name(chooser: &mut FileChooser, name: &str) {
    chooser.update(|inner| inner.current_name(name));
}

pub(crate) fn file_chooser_add_filter(chooser: &mut FileChooser, name: &str) {
    chooser.filters.push(Filter::named(name));
}

pub(crate) fn file_chooser_add_filter_pattern(chooser: &mut FileChooser, pattern: &str) {
    chooser.update_filter(|filter| filter.pattern(pattern));
}

pub(crate) fn file_chooser_add_filter_mime_type(chooser: &mut FileChooser, mime_type: &str) {
    chooser.update_filter(|filter| filter.mime_type(mime_type));
}
