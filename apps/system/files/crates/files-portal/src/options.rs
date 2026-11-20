use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use files_dialog::{Ask, Filter, Kind};
use rikka_types::WindowIdentifier;
use zbus::zvariant::{DeserializeDict, Type};

use crate::wire::{WireChoice, WireFilter};

#[derive(Debug, Default, DeserializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(crate) struct Options {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    directory: Option<bool>,
    filters: Option<Vec<WireFilter>>,
    current_filter: Option<WireFilter>,
    choices: Option<Vec<WireChoice>>,
    current_folder: Option<Vec<u8>>,
    current_name: Option<String>,
    current_file: Option<Vec<u8>>,
    files: Option<Vec<Vec<u8>>>,
}

impl Options {
    pub(crate) fn into_ask(self, kind: Kind, title: String, parent_window: &str) -> Ask {
        let (filters, current_filter) = self.filter_list();

        let editing = self.current_file.as_deref().and_then(path);
        let mut folder = editing
            .as_deref()
            .and_then(|at| at.parent())
            .map(PathBuf::from);
        let mut name = editing
            .as_deref()
            .and_then(|at| at.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(at) = self.current_folder.as_deref().and_then(path) {
            folder = Some(at);
        }
        if let Some(suggested) = self.current_name {
            name = suggested;
        }

        let base = Ask::new(kind);
        Ask {
            title,
            parent: parent(parent_window),
            modal: self.modal.unwrap_or(base.modal),
            accept_label: self.accept_label.as_deref().map(plain).unwrap_or_default(),
            folder,
            name,
            names: self
                .files
                .unwrap_or_default()
                .iter()
                .map(|name| String::from_utf8_lossy(until_nul(name)).into_owned())
                .collect(),
            filters,
            current_filter,
            choices: self
                .choices
                .unwrap_or_default()
                .iter()
                .map(Into::into)
                .collect(),
            multiple: self.multiple.unwrap_or(false),
            directory: self.directory.unwrap_or(false),
            ..base
        }
    }

    fn filter_list(&self) -> (Vec<Filter>, Option<usize>) {
        let mut filters: Vec<Filter> = self
            .filters
            .iter()
            .flatten()
            .map(Filter::from)
            .filter(|filter| !filter.rules.is_empty())
            .collect();
        let current = self
            .current_filter
            .as_ref()
            .map(Filter::from)
            .filter(|filter| !filter.rules.is_empty())
            .map(
                |filter| match filters.iter().position(|known| *known == filter) {
                    Some(index) => index,
                    None => {
                        filters.push(filter);
                        filters.len() - 1
                    }
                },
            );
        (filters, current)
    }
}

fn path(bytes: &[u8]) -> Option<PathBuf> {
    let bytes = until_nul(bytes);
    (!bytes.is_empty()).then(|| PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

fn until_nul(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|byte| *byte == 0) {
        Some(end) => &bytes[..end],
        None => bytes,
    }
}

fn plain(label: &str) -> String {
    let mut plain = String::with_capacity(label.len());
    let mut rest = label.chars();
    while let Some(c) = rest.next() {
        if c != '_' {
            plain.push(c);
        } else if rest.as_str().starts_with('_') {
            rest.next();
            plain.push('_');
        }
    }
    plain
}

fn parent(parent_window: &str) -> Option<WindowIdentifier> {
    if parent_window.is_empty() {
        return None;
    }
    parent_window
        .parse()
        .inspect_err(|error| tracing::warn!(parent_window, "ignoring parent window: {error}"))
        .ok()
}
