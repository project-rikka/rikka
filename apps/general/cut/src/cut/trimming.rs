use cut_project::{ClipId, Edge, Edit};
use rikka_app::prelude::*;

use super::Cut;
use crate::trimmer::Trimmer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditDirection {
    Previous,
    Next,
}

impl Cut {
    pub(crate) fn trimmer(&self) -> Trimmer {
        self.trimmer
    }

    pub(crate) fn set_trimmer(&mut self, trimmer: Trimmer) {
        if self.trimmer == trimmer {
            skip_view();
            return;
        }
        self.trimmer = trimmer;
    }

    pub(crate) fn peek_at(&mut self, frame: i64) {
        skip_view();
        self.live.show_frame(frame);
    }

    pub(crate) fn ripple_trim_to_playhead(&mut self, direction: EditDirection) {
        let frame = self.transport.frame();
        let points = self.project().edit_frames();
        let span = match direction {
            EditDirection::Previous => points
                .into_iter()
                .rev()
                .find(|point| *point < frame)
                .map(|point| point..frame),
            EditDirection::Next => points
                .into_iter()
                .find(|point| *point > frame)
                .map(|point| frame..point),
        };
        let Some(span) = span else {
            skip_view();
            return;
        };
        let start = span.start;
        self.edit(Edit::Extract(span));
        if direction == EditDirection::Previous {
            self.place_playhead(start);
        }
    }

    pub(crate) fn trim_to_playhead(&mut self, direction: EditDirection) {
        let frame = self.transport.frame();
        let edits: Vec<Edit> = self
            .targets_at(frame)
            .into_iter()
            .filter_map(|id| {
                let (_, clip) = self.project().locate(id)?;
                let (edge, by) = match direction {
                    EditDirection::Previous => (Edge::Head, frame - clip.start()),
                    EditDirection::Next => (Edge::Tail, frame - clip.end()),
                };
                (by != 0).then_some(Edit::Trim {
                    clips: vec![id],
                    edge,
                    by,
                })
            })
            .collect();
        self.commit_edits(edits);
    }

    pub(crate) fn extend_to_playhead(&mut self) {
        let frame = self.transport.frame();
        if let Some(edit_point) = self.selected_edit_point() {
            let Some(joint) = self.project().joint(edit_point) else {
                return;
            };
            self.edit(Edit::Roll {
                edit_point,
                by: frame - joint.at,
            });
            return;
        }
        let edits: Vec<Edit> = self
            .session
            .selected
            .iter()
            .filter_map(|id| {
                let (_, clip) = self.project().locate(*id)?;
                let head = frame - clip.start();
                let tail = frame - clip.end();
                let (edge, by) = match head.abs() < tail.abs() {
                    true => (Edge::Head, head),
                    false => (Edge::Tail, tail),
                };
                (by != 0).then_some(Edit::Trim {
                    clips: vec![*id],
                    edge,
                    by,
                })
            })
            .collect();
        self.commit_edits(edits);
    }

    fn targets_at(&self, frame: i64) -> Vec<ClipId> {
        let under = self.project().clips_under(frame);
        match self.session.selected.is_empty() {
            true => under,
            false => under
                .into_iter()
                .filter(|id| self.session.selected.contains(id))
                .collect(),
        }
    }
}
