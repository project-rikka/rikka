use std::path::PathBuf;

use cut_project::{Edit, Media};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_ui::DragOffer;

use super::Cut;
use super::files::is_project;
use crate::probe::{self, ProbeError};
use crate::wording;

type ProbeResults = Vec<Result<Media, (PathBuf, ProbeError)>>;

pub(super) fn footage() -> Filter {
    Filter::named(tr!("cut.import.footage").to_string())
        .mime_type("video/*")
        .mime_type("audio/*")
}

impl Cut {
    pub(crate) fn select_footage(&mut self) {
        let chooser = Chooser::open(tr!("cut.import.title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("cut.import.accept").to_string())
            .multiple(true)
            .filter(footage());
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            |cut: &mut Cut, selected: rikka_dialog::Result<ChooserResponse>| match selected {
                Ok(ChooserResponse::Files(paths)) => {
                    cut.folder = paths
                        .first()
                        .and_then(|path| path.parent())
                        .map(PathBuf::from);
                    cut.import(paths, None);
                }
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(trouble) => {
                    cut.notice = Some(tr!("cut.import.unasked", reason = &trouble.to_string()));
                }
            },
        );
    }

    pub(crate) fn dropped(offer: DragOffer, at: Option<(i64, usize)>) {
        spawn_blocking(
            move || {
                offer
                    .uris()
                    .iter()
                    .filter_map(|uri| rikka_open::uri_to_path(uri))
                    .collect::<Vec<PathBuf>>()
            },
            move |cut: &mut Cut, paths: Vec<PathBuf>| cut.arrive(paths, at),
        );
    }

    pub(crate) fn arrive(&mut self, paths: Vec<PathBuf>, at: Option<(i64, usize)>) {
        let (projects, footage): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|path| is_project(path));
        if let Some(project) = projects.into_iter().next() {
            self.ask_to_open_project(project);
        }
        if !footage.is_empty() {
            self.import(footage, at);
        }
    }

    pub(crate) fn import(&mut self, paths: Vec<PathBuf>, at: Option<(i64, usize)>) {
        let (at, layer) = at.unwrap_or((self.transport.frame(), 0));
        self.importing += 1;
        spawn_blocking(
            move || {
                paths
                    .into_iter()
                    .map(|path| probe::footage(&path).map_err(|error| (path, error)))
                    .collect::<ProbeResults>()
            },
            move |cut: &mut Cut, results: ProbeResults| cut.import_probed(results, at, layer),
        );
    }

    fn import_probed(&mut self, results: ProbeResults, at: i64, layer: usize) {
        self.importing -= 1;
        let mut footage = Vec::with_capacity(results.len());
        let mut refused = Vec::new();
        for result in results {
            match result {
                Ok(media) => footage.push(media),
                Err(error) => refused.push(error),
            }
        }
        if !footage.is_empty() {
            self.edit(Edit::Place {
                media: footage,
                at,
                layer,
            });
        }
        if let Some((path, error)) = refused.first() {
            self.notice = Some(wording::unimported(path, error, refused.len()));
        }
    }
}
