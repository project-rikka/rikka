use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{Applied, ClipId, Edit, Error, Point, Project};
use rikka_app::prelude::*;

use super::Cut;
use crate::wording;

impl Cut {
    pub(crate) fn edit(&mut self, edit: Edit) {
        self.edit_selecting(edit);
    }

    pub(crate) fn edit_selecting(&mut self, edit: Edit) -> Option<Vec<ClipId>> {
        self.edit_applied(edit)?.selected
    }

    pub(crate) fn edit_applied(&mut self, edit: Edit) -> Option<Applied> {
        self.commit_text();
        let fitting = self.project().is_empty();
        match self.document.apply(&edit) {
            Ok(applied) => {
                if let Some(selected) = &applied.selected {
                    self.session.selected = Arc::new(selected.iter().copied().collect());
                    self.session.edit_point = None;
                }
                self.changed();
                if fitting && !self.project().is_empty() {
                    self.scale.fit(self.project().end());
                }
                Some(applied)
            }
            Err(Error::NothingToSplit) => {
                skip_view();
                None
            }
            Err(error) => {
                self.notice = Some(wording::refused(&error));
                None
            }
        }
    }

    pub(crate) fn amend(&mut self, edit: Edit) {
        match self.document.amend(&edit) {
            Ok(_) => self.changed(),
            Err(error) => self.notice = Some(wording::refused(&error)),
        }
    }

    pub(crate) fn shown_project(&self) -> Arc<Project> {
        self.live.project_or(self.project())
    }

    pub(crate) fn changed(&mut self) {
        let project = Arc::clone(self.project());
        self.transport.set_rate(project.rate());
        if self
            .session
            .selected
            .iter()
            .any(|id| project.locate(*id).is_none())
        {
            let remaining: BTreeSet<ClipId> = self
                .session
                .selected
                .iter()
                .copied()
                .filter(|id| project.locate(*id).is_some())
                .collect();
            self.session.selected = Arc::new(remaining);
        }
        self.session.edit_point = self
            .session
            .edit_point
            .and_then(|edit_point| project.canonical_edit_point(edit_point));
        self.sync_writing();
        self.sync_marking();
        self.look_for_missing();
    }

    pub(crate) fn undo(&mut self) {
        self.commit_marker();
        self.commit_text();
        self.end_transition_preview();
        if self.document.undo().is_some() {
            self.changed();
        }
    }

    pub(crate) fn redo(&mut self) {
        self.commit_marker();
        self.commit_text();
        self.end_transition_preview();
        if self.document.redo().is_some() {
            self.changed();
        }
    }

    pub(crate) fn select(&mut self, clips: Vec<ClipId>) {
        let editable: BTreeSet<ClipId> = self.project().editable(&clips).into_iter().collect();
        if editable == *self.session.selected && self.session.edit_point.is_none() {
            skip_view();
            return;
        }
        self.end_transition_preview();
        self.session.selected = Arc::new(editable);
        self.session.edit_point = None;
        self.session.popup = None;
        self.forget_keys();
        self.sync_writing();
    }

    pub(crate) fn select_all(&mut self) {
        let all: Vec<ClipId> = self.project().clips().map(|(_, clip)| clip.id()).collect();
        self.select(all);
    }

    pub(crate) fn delete(&mut self, ripple: bool) {
        let clips: Vec<ClipId> = self.session.selected.iter().copied().collect();
        if clips.is_empty() {
            self.remove_transition();
            return;
        }
        self.edit(match ripple {
            true => Edit::RippleDelete(clips),
            false => Edit::Delete(clips),
        });
    }

    pub(crate) fn split_at_playhead(&mut self) {
        let at = self.transport.frame();
        let under = self.project().clips_under(at);
        let clips = match self.session.selected.is_empty() {
            true => under,
            false => under
                .into_iter()
                .filter(|id| self.session.selected.contains(id))
                .collect(),
        };
        self.edit(Edit::Split { clips, at });
    }

    pub(crate) fn mark(&mut self, point: Point) {
        let at = self.transport.frame();
        self.edit(Edit::Mark { point, at });
    }

    pub(crate) fn clear_marks(&mut self) {
        self.edit(Edit::ClearMarks);
    }
}
