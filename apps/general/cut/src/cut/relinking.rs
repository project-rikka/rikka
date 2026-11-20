use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cut_project::{Clip, Edit, Media, MediaId, Relink};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};

use super::Cut;
use super::importing::footage;
use super::sheet::{Sheet, draft, draft_mut};
use crate::relink::{self, Busy, Outcome, Refusal, Relinking, Searched};
use crate::wording;

enum Chosen<T> {
    Done(T),
    Cancelled,
    Unasked(String),
}

fn chosen<T>(
    chooser: Chooser,
    folder: Option<PathBuf>,
    then: impl FnOnce(PathBuf) -> T,
) -> Chosen<T> {
    let chooser = match folder.as_deref().and_then(nearest_folder) {
        Some(folder) => chooser.current_folder(folder),
        None => chooser,
    };
    match chooser.run() {
        Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
            Some(path) => Chosen::Done(then(path)),
            None => Chosen::Cancelled,
        },
        Ok(ChooserResponse::Cancelled) => Chosen::Cancelled,
        Err(trouble) => Chosen::Unasked(trouble.to_string()),
    }
}

fn nearest_folder(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|folder| folder.is_dir())
        .map(Path::to_path_buf)
}

impl Cut {
    pub(crate) fn relinking(&self) -> Option<&Relinking> {
        draft(&self.sheet)
    }

    pub(crate) fn can_relink(&self) -> bool {
        !self.offline.is_empty()
    }

    pub(crate) fn can_replace_media(&self) -> bool {
        self.selected_clip().and_then(Clip::media).is_some()
    }

    pub(crate) fn open_relink(&mut self) {
        if self.is_modal() || self.offline.is_empty() {
            return;
        }
        self.commit_text();
        self.close_marker();
        self.pause();
        self.sheet = Some(Sheet::Relink(Relinking::new(self.project(), &self.offline)));
    }

    pub(crate) fn close_relink(&mut self) {
        self.sheet = None;
    }

    pub(crate) fn locate_media(&mut self, media: MediaId) {
        let project = Arc::clone(self.project());
        let fallback = self.folder.clone();
        let Some(relinking) =
            draft_mut::<Relinking>(&mut self.sheet).filter(|draft| draft.busy.is_none())
        else {
            return;
        };
        let Some(entry) = relinking.entry(media) else {
            return;
        };
        let original = entry.original.clone();
        let others = relinking.unresolved();
        relinking.busy = Some(Busy::Locating(media));
        let chooser =
            Chooser::open(tr!("cut.relink.locate_title", name = &original.name()).to_string())
                .parent(window_identifier())
                .accept_label(tr!("cut.relink.locate_accept").to_string())
                .filter(footage());
        let folder = original.path().parent().map(Path::to_path_buf).or(fallback);
        spawn_blocking(
            move || {
                chosen(chooser, folder, |path| {
                    relink::located(&project, media, path, &others)
                })
            },
            |cut: &mut Cut, chosen: Chosen<Vec<Outcome>>| {
                cut.settle_relink(chosen, |relinking, outcomes| {
                    relinking.settle(outcomes);
                    None
                });
            },
        );
    }

    pub(crate) fn search_media(&mut self) {
        let project = Arc::clone(self.project());
        let folder = self.folder.clone();
        let Some(relinking) =
            draft_mut::<Relinking>(&mut self.sheet).filter(|draft| draft.busy.is_none())
        else {
            return;
        };
        let wanted = relinking.unresolved();
        relinking.busy = Some(Busy::Searching);
        let chooser = Chooser::open(tr!("cut.relink.search_title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("cut.relink.search_accept").to_string())
            .directory(true);
        spawn_blocking(
            move || {
                chosen(chooser, folder, |root| {
                    relink::searched(&project, &root, &wanted).map_err(|error| (root, error))
                })
            },
            |cut: &mut Cut, chosen: Chosen<Result<Searched, (PathBuf, io::Error)>>| {
                cut.settle_relink(chosen, |relinking, searched| match searched {
                    Ok(searched) => {
                        let found = relinking.settle(searched.outcomes);
                        Some(wording::searched(found, searched.unreadable))
                    }
                    Err((root, error)) => Some(tr!(
                        "cut.relink.unsearchable",
                        folder = &root.display().to_string(),
                        reason = &error.to_string()
                    )),
                });
            },
        );
    }

    fn settle_relink<T>(
        &mut self,
        chosen: Chosen<T>,
        settle: impl FnOnce(&mut Relinking, T) -> Option<Label>,
    ) {
        let Some(relinking) = draft_mut::<Relinking>(&mut self.sheet) else {
            skip_view();
            return;
        };
        relinking.busy = None;
        match chosen {
            Chosen::Done(done) => relinking.note = settle(relinking, done),
            Chosen::Cancelled => {}
            Chosen::Unasked(reason) => {
                relinking.note = Some(tr!("cut.import.unasked", reason = &reason));
            }
        }
    }

    pub(crate) fn relink_found(&mut self) {
        let Some(relinks) = draft::<Relinking>(&self.sheet)
            .filter(|relinking| relinking.busy.is_none())
            .map(Relinking::relinks)
            .filter(|relinks| !relinks.is_empty())
        else {
            return;
        };
        self.sheet = None;
        self.edit(Edit::Relink(relinks));
    }

    pub(crate) fn replace_media(&mut self) {
        let Some(media) = self.selected_clip().and_then(Clip::media) else {
            return;
        };
        if self.offline.contains(&media) {
            self.open_relink();
            return;
        }
        let Some(original) = self.project().media(media).cloned() else {
            return;
        };
        if self.is_modal() {
            return;
        }
        let project = Arc::clone(self.project());
        let chooser = Chooser::open(tr!("cut.replace.title", name = &original.name()).to_string())
            .parent(window_identifier())
            .accept_label(tr!("cut.replace.accept").to_string())
            .filter(footage());
        let folder = original.path().parent().map(Path::to_path_buf);
        spawn_blocking(
            move || {
                chosen(chooser, folder, |path| {
                    relink::tried(&project, media, &path).map_err(|refusal| (path, refusal))
                })
            },
            move |cut: &mut Cut, chosen: Chosen<Result<Media, (PathBuf, Refusal)>>| match chosen {
                Chosen::Done(Ok(to)) => cut.edit(Edit::Relink(vec![Relink { media, to }])),
                Chosen::Done(Err((path, refusal))) => {
                    cut.notice = Some(tr!(
                        "cut.replace.refused",
                        name = &wording::file_name(&path),
                        reason = &wording::relink_refusal(&refusal).to_string()
                    ));
                }
                Chosen::Cancelled => skip_view(),
                Chosen::Unasked(reason) => {
                    cut.notice = Some(tr!("cut.import.unasked", reason = &reason));
                }
            },
        );
    }
}
