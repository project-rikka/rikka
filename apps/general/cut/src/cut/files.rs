use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cut_project::{Document, EXTENSION, MediaId, Project};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};

use super::Cut;
use super::session::Session;
use super::writing::Draft;

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingAction {
    Close,
    StartOver,
    Open(Option<PathBuf>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnsavedPrompt {
    action: PendingAction,
    saving: bool,
}

impl UnsavedPrompt {
    pub(crate) fn is_saving(&self) -> bool {
        self.saving
    }
}

fn projects() -> Filter {
    Filter::named(tr!("cut.file.projects").to_string()).pattern(format!("*.{EXTENSION}"))
}

pub(crate) fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
}

impl Cut {
    pub(crate) fn name(&self) -> String {
        match self.document.path().and_then(Path::file_stem) {
            Some(stem) => stem.to_string_lossy().into_owned(),
            None => tr!("cut.untitled").to_string(),
        }
    }

    pub(crate) fn is_modified(&self) -> bool {
        self.document.is_modified() || self.session.writing.as_ref().is_some_and(Draft::is_dirty)
    }

    pub(crate) fn unsaved_prompt(&self) -> Option<&UnsavedPrompt> {
        self.unsaved_prompt.as_ref()
    }

    pub(crate) fn is_asking(&self) -> bool {
        self.unsaved_prompt.is_some()
    }

    fn confirm_unsaved(&mut self, action: PendingAction) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        match self.is_modified() {
            true => {
                self.pause();
                self.unsaved_prompt = Some(UnsavedPrompt {
                    action,
                    saving: false,
                });
            }
            false => self.perform(action),
        }
    }

    pub(crate) fn ask_to_close(&mut self) {
        self.close_export_sheet();
        self.confirm_unsaved(PendingAction::Close);
    }

    pub(crate) fn ask_to_start_over(&mut self) {
        self.confirm_unsaved(PendingAction::StartOver);
    }

    pub(crate) fn ask_to_open(&mut self) {
        self.confirm_unsaved(PendingAction::Open(None));
    }

    pub(crate) fn ask_to_open_project(&mut self, path: PathBuf) {
        self.confirm_unsaved(PendingAction::Open(Some(path)));
    }
    pub(crate) fn answer(&mut self, answer: Unsaved) {
        let Some(unsaved_prompt) = self.unsaved_prompt.clone() else {
            return;
        };
        match answer {
            Unsaved::Cancel => self.unsaved_prompt = None,
            Unsaved::Discard => {
                self.unsaved_prompt = None;
                self.perform(unsaved_prompt.action);
            }
            Unsaved::Save => {
                self.unsaved_prompt = Some(UnsavedPrompt {
                    saving: true,
                    ..unsaved_prompt
                });
                self.save();
            }
        }
    }

    fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::Close => request_window(WindowRequest::Close),
            PendingAction::StartOver => self.hold(Document::new()),
            PendingAction::Open(Some(path)) => self.open_project(path),
            PendingAction::Open(None) => self.select_project(),
        }
    }

    fn saved_then(&mut self, saved: bool) {
        let Some(unsaved_prompt) = self.unsaved_prompt.take() else {
            return;
        };
        match saved {
            true => self.perform(unsaved_prompt.action),
            false => self.unsaved_prompt = None,
        }
    }

    pub(crate) fn save(&mut self) {
        match self.document.path() {
            Some(path) => {
                let path = path.to_path_buf();
                self.write(path);
            }
            None => self.save_as(),
        }
    }

    pub(crate) fn save_as(&mut self) {
        let named = format!("{}.{EXTENSION}", self.name());
        let chooser = Chooser::save(tr!("cut.save.title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("cut.save.accept").to_string())
            .current_name(named)
            .filter(projects());
        let chooser = match self
            .document
            .path()
            .and_then(Path::parent)
            .or(self.folder.as_deref())
        {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            |cut: &mut Cut, selected: rikka_dialog::Result<ChooserResponse>| match selected {
                Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                    Some(path) => cut.write(with_extension(path)),
                    None => cut.saved_then(false),
                },
                Ok(ChooserResponse::Cancelled) => cut.saved_then(false),
                Err(trouble) => {
                    cut.notice = Some(tr!("cut.save.unasked", reason = &trouble.to_string()));
                    cut.saved_then(false);
                }
            },
        );
    }

    fn write(&mut self, path: PathBuf) {
        self.commit_marker();
        self.commit_text();
        let project = Arc::clone(self.project());
        let revision = self.document.revision();
        let target = path.clone();
        spawn_blocking(
            move || cut_project::save(&project, &target),
            move |cut: &mut Cut, result: cut_project::Result<()>| match result {
                Ok(()) => {
                    cut.folder = path.parent().map(Path::to_path_buf);
                    cut.document.saved(path.clone(), revision);
                    cut.saved_then(true);
                }
                Err(error) => {
                    cut.notice = Some(tr!("cut.save.failed", reason = &error.to_string()));
                    cut.saved_then(false);
                }
            },
        );
    }

    fn select_project(&mut self) {
        let chooser = Chooser::open(tr!("cut.open.title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("cut.open.accept").to_string())
            .filter(projects());
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            |cut: &mut Cut, selected: rikka_dialog::Result<ChooserResponse>| match selected {
                Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                    Some(path) => cut.open_project(path),
                    None => skip_view(),
                },
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(trouble) => {
                    cut.notice = Some(tr!("cut.open.unasked", reason = &trouble.to_string()));
                }
            },
        );
    }

    fn open_project(&mut self, path: PathBuf) {
        spawn_blocking(
            move || Document::open(path),
            |cut: &mut Cut, result: cut_project::Result<Document>| match result {
                Ok(document) => {
                    cut.folder = document
                        .path()
                        .and_then(Path::parent)
                        .map(Path::to_path_buf);
                    cut.hold(document);
                    cut.scale.fit(cut.project().end());
                }
                Err(error) => {
                    cut.notice = Some(tr!("cut.open.failed", reason = &error.to_string()));
                }
            },
        );
    }

    fn hold(&mut self, document: Document) {
        self.end_transition_preview();
        self.pause();
        self.session = Session::default();
        self.sheet = None;
        self.live.settle();
        for path in linked(self.project()).values() {
            self.media_previews.forget(path);
        }
        self.document = document;
        self.offline = Arc::default();
        self.linked = None;
        self.asking_relink = true;
        self.notice = None;
        self.transport.seek(0);
        self.changed();
    }

    pub(crate) fn look_for_missing(&mut self) {
        let linked = linked(self.project());
        if self.linked.as_ref() == Some(&linked) {
            return;
        }
        self.linked = Some(linked.clone());
        spawn_blocking(
            move || {
                let missing = missing(&linked);
                (linked, missing)
            },
            |cut: &mut Cut, (linked, missing): (Linked, BTreeSet<MediaId>)| {
                if cut.linked.as_ref() != Some(&linked) {
                    skip_view();
                    return;
                }
                let asking = std::mem::take(&mut cut.asking_relink);
                let mut returned = false;
                for path in cut
                    .offline
                    .iter()
                    .filter(|media| !missing.contains(media))
                    .filter_map(|media| linked.get(media))
                {
                    cut.media_previews.forget(path);
                    returned = true;
                }
                let changed = missing != *cut.offline;
                if changed {
                    cut.offline = Arc::new(missing);
                    cut.watch_offline(&linked);
                }
                if returned {
                    cut.playback.reopen_offline();
                }
                match asking && !cut.offline.is_empty() {
                    true => cut.open_relink(),
                    false if !changed => skip_view(),
                    false => {}
                }
            },
        );
    }

    fn watch_offline(&mut self, linked: &Linked) {
        self.offline_watch = None;
        let paths: Vec<PathBuf> = self
            .offline
            .iter()
            .filter_map(|media| linked.get(media).cloned())
            .collect();
        if paths.is_empty() {
            return;
        }
        let sender = self.sender.clone();
        let watch = rikka_xdg::Watch::paths(paths, move || {
            let _ = sender.send(Cut::recheck_media);
        });
        match watch {
            Ok(watch) => self.offline_watch = Some(watch),
            Err(error) => tracing::warn!(%error, "cannot watch offline media"),
        }
    }

    fn recheck_media(&mut self) {
        self.linked = None;
        self.look_for_missing();
        skip_view();
    }
}

pub(crate) type Linked = BTreeMap<MediaId, PathBuf>;

fn linked(project: &Project) -> Linked {
    let used: BTreeSet<MediaId> = project
        .clips()
        .filter_map(|(_, clip)| clip.media())
        .collect();
    used.into_iter()
        .filter_map(|media| Some((media, project.media(media)?.path().to_path_buf())))
        .collect()
}

fn missing(linked: &Linked) -> BTreeSet<MediaId> {
    linked
        .iter()
        .filter(|(_, path)| !path.is_file())
        .map(|(media, _)| *media)
        .collect()
}

fn with_extension(path: PathBuf) -> PathBuf {
    match is_project(&path) {
        true => path,
        false => {
            let mut named = path.into_os_string();
            named.push(".");
            named.push(EXTENSION);
            PathBuf::from(named)
        }
    }
}
