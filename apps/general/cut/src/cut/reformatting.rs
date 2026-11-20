use cut_project::Edit;

use super::Cut;
use super::sheet::{Sheet, draft, draft_mut};
use crate::reformat::Reformatting;

impl Cut {
    pub(crate) fn reformatting(&self) -> Option<&Reformatting> {
        draft(&self.sheet)
    }

    pub(crate) fn open_reformat(&mut self) {
        if self.is_modal() {
            return;
        }
        self.commit_text();
        self.close_marker();
        self.pause();
        self.sheet = Some(Sheet::Reformat(Reformatting::new(self.project().format())));
    }

    pub(crate) fn close_reformat(&mut self) {
        self.sheet = None;
    }

    pub(crate) fn tune_reformat(&mut self, change: impl FnOnce(&mut Reformatting)) {
        if let Some(reformatting) = draft_mut(&mut self.sheet) {
            change(reformatting);
        }
    }

    pub(crate) fn reformat(&mut self) {
        let Some(Sheet::Reformat(reformatting)) = self
            .sheet
            .take_if(|sheet| matches!(sheet, Sheet::Reformat(_)))
        else {
            return;
        };
        let (from, to) = (reformatting.original, reformatting.format);
        if from == to {
            return;
        }
        let frame = self.transport.frame();
        let seconds = frame as f64 / from.rate.per_second();
        if self.edit_applied(Edit::Reformat(to)).is_none() {
            return;
        }
        if from.rate != to.rate {
            self.scale.fit(self.project().end());
            self.place_playhead((seconds * to.rate.per_second()).round() as i64);
        }
    }
}
