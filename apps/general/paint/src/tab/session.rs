use std::path::PathBuf;

use paint_document::{Edit, LayerId, Region, Saving};
use rikka_app::prelude::Label;
use rikka_image_edit::Recipe;

use super::Tab;
use crate::session::{Dialog, Kind, Outcome, Session};
use crate::wording;

enum Applied {
    Develop { layer: LayerId, recipe: Recipe },
    Edit(Edit),
    Save { path: PathBuf, saving: Saving },
}

impl Tab {
    pub(crate) fn begin(&mut self, kind: Kind) {
        let Some(session) = self.context().map(|context| kind.session(&context)) else {
            return;
        };
        self.begin_session(session);
    }

    pub(crate) fn begin_session(&mut self, session: Session) {
        if !self.edits() {
            return;
        }
        let measures = session.dialog().wants_histogram();
        let layer = session.layer();
        self.previews.floor = self.previews.request_id;
        self.session = Some(session);
        if let (true, Some(worker), Some(layer)) = (measures, &self.worker, layer) {
            self.measured.request_id += 1;
            self.measured.histogram = None;
            let region = self.selection.region();
            let within = (!region.is_empty()).then(|| Region::clone(&region));
            worker.measure(self.measured.request_id, layer, within);
        }
        self.refresh_preview();
    }

    pub(crate) fn tune<D: Dialog>(&mut self, change: impl FnOnce(&mut D)) -> bool {
        let tuned = self
            .session
            .as_mut()
            .is_some_and(|session| session.tune(change));
        if tuned {
            self.refresh_preview();
        }
        tuned
    }

    pub(crate) fn reset_session(&mut self) {
        let Some(mut session) = self.session.take() else {
            return;
        };
        if let Some(context) = self.context() {
            session.reset(&context);
        }
        self.session = Some(session);
        self.refresh_preview();
    }

    pub(crate) fn preview_session(&mut self, previewing: bool) {
        let Some(session) = &mut self.session else {
            return;
        };
        session.set_previewing(previewing);
        match previewing {
            true => self.refresh_preview(),
            false => {
                self.previews.floor = self.previews.request_id;
                self.picture.unpreview();
            }
        }
    }

    pub(crate) fn rescaled(&mut self) -> bool {
        if self.relift() {
            return true;
        }
        let previewing = self.session.as_ref().is_some_and(Session::is_previewing);
        if !previewing || self.camera.scale() == self.previews.scale {
            return false;
        }
        self.refresh_preview();
        true
    }

    fn refresh_preview(&mut self) {
        let previewing = self.session.as_ref().is_some_and(Session::is_previewing);
        if !previewing {
            return;
        }
        let Some((layer, recipe)) = self.preview_order() else {
            self.end_preview(None);
            return;
        };
        let scale = self.camera.scale();
        if let Some(worker) = &self.worker {
            self.previews.request_id += 1;
            self.previews.scale = scale;
            worker.preview(self.previews.request_id, layer, recipe, scale);
        }
    }

    fn preview_order(&self) -> Option<(LayerId, Recipe)> {
        let session = self.session.as_ref()?;
        let context = self.context()?;
        if session.refusal(&context).is_some() {
            return None;
        }
        let Some(Outcome::Develop(recipe)) = session.dialog().outcome(&context) else {
            return None;
        };
        Some((session.layer()?, context.masked(&recipe).ok()?))
    }

    pub(crate) fn develop(&mut self, recipe: &Recipe) -> Result<(), Label> {
        let (layer, masked) = {
            let Some(context) = self.context() else {
                return Ok(());
            };
            if let Some(refusal) = context.develop_refusal() {
                return Err(refusal);
            }
            let masked = context
                .masked(recipe)
                .map_err(|error| wording::error_message(&error))?;
            (context.layer.map(|layer| layer.id), masked)
        };
        if let Some(layer) = layer
            && self.edits()
        {
            self.order(Edit::Develop {
                layer,
                recipe: masked,
            });
        }
        Ok(())
    }

    pub(crate) fn cancel_session(&mut self) {
        if self.session.take().is_some() {
            self.end_preview(None);
        }
    }

    pub(crate) fn apply_session(&mut self) {
        let Some(applied) = self.applied() else {
            return;
        };
        self.session = None;
        match applied {
            Applied::Develop { layer, recipe } => {
                self.order(Edit::Develop { layer, recipe });
                self.end_preview(Some(self.ordered));
            }
            Applied::Edit(edit) => {
                self.end_preview(None);
                self.order_reshaping(edit);
            }
            Applied::Save { path, saving } => {
                self.end_preview(None);
                self.save_to(path, saving);
            }
        }
    }

    fn applied(&self) -> Option<Applied> {
        let session = self.session.as_ref()?;
        let context = self.context()?;
        if session.refusal(&context).is_some() {
            return None;
        }
        Some(match session.dialog().outcome(&context)? {
            Outcome::Develop(recipe) => Applied::Develop {
                layer: session.layer()?,
                recipe: context.masked(&recipe).ok()?,
            },
            Outcome::Edit(edit) => Applied::Edit(edit),
            Outcome::Save { path, saving } => Applied::Save { path, saving },
        })
    }

    pub(super) fn end_preview(&mut self, retire: Option<u64>) {
        let previewed = self.previews.request_id > self.previews.floor;
        self.previews.floor = self.previews.request_id;
        if !previewed {
            return;
        }
        if let Some(worker) = &self.worker {
            worker.end_preview();
        }
        match retire {
            Some(generation) => self.previews.retire = Some(generation),
            None => self.picture.unpreview(),
        }
    }

    pub(super) fn previewed(
        &mut self,
        preview: u64,
        tiles: paint_document::Result<Vec<paint_document::Tile>>,
    ) {
        if preview <= self.previews.floor {
            return;
        }
        if preview > self.previews.answered {
            self.previews.answered = preview;
            match tiles {
                Ok(tiles) => {
                    self.picture.preview(tiles);
                    self.failure = None;
                }
                Err(error) => self.failure = Some(wording::error_message(&error)),
            }
        }
    }
}
