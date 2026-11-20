use std::sync::Arc;

use cut_project::Edit;
use rikka_app::prelude::*;

use super::Cut;

impl Cut {
    pub(crate) fn preview_edit(&mut self, edit: &Edit) {
        self.preview_edits(std::slice::from_ref(edit));
    }

    pub(crate) fn preview_edits(&mut self, edits: &[Edit]) {
        skip_view();
        let mut previewed = Arc::clone(self.project());
        for edit in edits {
            match previewed.apply(edit) {
                Ok((project, _)) => previewed = Arc::new(project),
                Err(_) => return,
            }
        }
        match Arc::ptr_eq(&previewed, self.project()) {
            true => self.live.settle(),
            false => self
                .live
                .rework(self.document.project(), Arc::unwrap_or_clone(previewed)),
        }
    }

    pub(crate) fn commit_preview(&mut self) {
        skip_view();
        self.live.settle();
    }

    pub(crate) fn commit_edits(&mut self, edits: Vec<Edit>) {
        let mut begun = false;
        for edit in edits {
            let before = Arc::clone(self.project());
            match begun {
                false => self.edit(edit),
                true => self.amend(edit),
            }
            begun |= !Arc::ptr_eq(&before, self.project());
        }
        self.live.settle();
    }
}
