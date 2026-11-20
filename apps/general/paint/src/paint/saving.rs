use std::path::{Path, PathBuf};

use paint_document::FileFormat;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};

use super::Paint;
use crate::session::{SaveChoices, SaveOptions, Session};
use crate::tab::Tab;

const PHOTOSHOP: FileFormat = FileFormat::Photoshop { large: false };
const PNG: FileFormat = FileFormat::Picture(rikka_image::Format::Png);

impl Paint {
    pub(crate) fn can_save(&self) -> bool {
        self.edits() && self.active_tab().is_some_and(|tab| !tab.is_saving())
    }

    pub(crate) fn save(&mut self) {
        if let Some(tab) = self.active_tab() {
            let id = tab.id();
            self.save_tab(id);
        }
    }

    pub(crate) fn save_as(&mut self) {
        if let Some(tab) = self.active_tab() {
            let id = tab.id();
            self.show_save_dialog(id, true);
        }
    }

    pub(super) fn save_tab(&mut self, id: u64) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id() == id) else {
            return;
        };
        let known = tab
            .path()
            .and_then(|path| FileFormat::for_path(path).map(|format| (path.to_path_buf(), format)));
        match known {
            Some((path, format)) => {
                let saving = self.choices(format).saving();
                if let Some(tab) = self.tab_mut(id) {
                    tab.save_to(path, saving);
                }
            }
            None => self.show_save_dialog(id, false),
        }
    }

    pub(super) fn choices(&self, format: FileFormat) -> SaveChoices {
        self.savings.get(&format).copied().unwrap_or_default()
    }

    fn show_save_dialog(&mut self, id: u64, offers_options: bool) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id() == id) else {
            return;
        };
        let suggested = suggested_format(tab);
        let mut named = match tab.path().and_then(Path::file_stem) {
            Some(stem) => stem.to_string_lossy().into_owned(),
            None => tab.name(),
        };
        named.push('.');
        named.push_str(suggested.extensions().first().copied().unwrap_or("png"));
        let chooser = filters(suggested).into_iter().fold(
            Chooser::save(tr!("paint.save.title").to_string())
                .parent(window_identifier())
                .accept_label(tr!("paint.save.accept").to_string())
                .current_name(named),
            Chooser::filter,
        );
        let chooser = match tab.path().and_then(Path::parent).or(self.folder.as_deref()) {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            move |paint: &mut Paint, active_layer: rikka_dialog::Result<ChooserResponse>| {
                match active_layer {
                    Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                        Some(path) => {
                            paint.save_to(id, &with_extension(path, suggested), offers_options);
                        }
                        None => paint.abandon_question(id),
                    },
                    Ok(ChooserResponse::Cancelled) => paint.abandon_question(id),
                    Err(error) => {
                        paint.abandon_question(id);
                        paint.notice = Some(tr!("paint.save.unasked", reason = &error.to_string()));
                    }
                }
            },
        );
    }

    pub(crate) fn save_to(&mut self, id: u64, path: &Path, offers_options: bool) {
        let Some(format) = FileFormat::for_path(path) else {
            return;
        };
        let choices = self.choices(format);
        let Some(tab) = self.tab_mut(id) else {
            return;
        };
        if offers_options && SaveOptions::asks(format) {
            let options = SaveOptions::new(path.to_path_buf(), format, choices);
            let fresh = path.to_path_buf();
            let session = tab.context().map(|context| {
                Session::holding(
                    options,
                    move |_| SaveOptions::new(fresh.clone(), format, SaveChoices::default()),
                    &context,
                )
            });
            if let Some(session) = session {
                tab.begin_session(session);
            }
            return;
        }
        tab.save_to(path.to_path_buf(), choices.saving());
    }
}

fn suggested_format(tab: &Tab) -> FileFormat {
    tab.path()
        .and_then(FileFormat::for_path)
        .unwrap_or_else(|| match tab.facts().map(|facts| facts.format) {
            Some(FileFormat::Photoshop { large }) => FileFormat::Photoshop { large },
            _ if tab.layers().len() > 1 => PHOTOSHOP,
            _ => PNG,
        })
}

fn filters(first: FileFormat) -> Vec<Filter> {
    let mut formats = vec![first];
    formats.extend(
        FileFormat::writable()
            .into_iter()
            .filter(|format| format.mime_type() != first.mime_type()),
    );
    let mut filters: Vec<Filter> = Vec::new();
    let mut listed: Vec<&'static str> = Vec::new();
    for format in formats {
        if listed.contains(&format.mime_type()) {
            continue;
        }
        listed.push(format.mime_type());
        filters.push(Filter::named(format.name()).mime_type(format.mime_type()));
    }
    filters
}

fn with_extension(path: PathBuf, suggested: FileFormat) -> PathBuf {
    if FileFormat::for_path(&path).is_some() {
        return path;
    }
    let mut named = path.into_os_string();
    named.push(".");
    named.push(suggested.extensions().first().copied().unwrap_or("png"));
    PathBuf::from(named)
}
