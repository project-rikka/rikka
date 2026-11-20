use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use cut_project::{Edit, EditPoint, Transition, TransitionAlignment, TransitionKind};
use rikka_app::prelude::*;
use rikka_keyframe::Easing;

use super::{Cut, Popup};
use crate::easing::Picking;

const LEAD: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Side {
    #[default]
    Properties,
    Transitions,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransitionPreview {
    span: Range<i64>,
    home: i64,
}

impl TransitionPreview {
    pub(crate) fn wraps(&self, frame: f64) -> Option<i64> {
        (frame >= self.span.end as f64).then_some(self.span.start)
    }
}

impl Cut {
    pub(crate) fn side(&self) -> Side {
        self.side
    }

    pub(crate) fn show_side(&mut self, side: Side) {
        self.side = side;
    }

    pub(crate) fn selected_edit_point(&self) -> Option<EditPoint> {
        self.session.edit_point
    }

    pub(crate) fn transition_preview(&self) -> Option<&TransitionPreview> {
        self.session.transition_preview.as_ref()
    }

    pub(crate) fn select_edit_point(&mut self, edit_point: EditPoint) {
        let Some(edit_point) = self.project().canonical_edit_point(edit_point) else {
            skip_view();
            return;
        };
        if self.session.edit_point == Some(edit_point) && self.session.selected.is_empty() {
            skip_view();
            return;
        }
        self.end_transition_preview();
        self.session.selected = Arc::default();
        self.session.popup = None;
        self.forget_keys();
        self.sync_writing();
        self.session.edit_point = Some(edit_point);
        self.side = match self.held_transition().is_some() {
            true => Side::Properties,
            false => Side::Transitions,
        };
    }

    pub(crate) fn held_transition(&self) -> Option<(EditPoint, Transition)> {
        let project = self.project();
        let edit_point = project.canonical_edit_point(self.session.edit_point?)?;
        Some((edit_point, *project.transition(edit_point)?))
    }

    pub(crate) fn transition_targets(&self) -> Vec<EditPoint> {
        match self.session.edit_point {
            Some(edit_point) => vec![edit_point],
            None => self.project().edit_points_among(&self.session.selected),
        }
    }

    pub(crate) fn transit(&mut self, kind: TransitionKind) {
        let edit_points = self.transition_targets();
        self.transit_edit_points(edit_points, kind, None);
    }

    pub(crate) fn transit_edit_points(
        &mut self,
        edit_points: Vec<EditPoint>,
        kind: TransitionKind,
        at: Option<TransitionAlignment>,
    ) {
        self.end_transition_preview();
        if edit_points.is_empty() {
            skip_view();
            return;
        }
        let mut grouped: Vec<(Transition, Vec<EditPoint>)> = Vec::new();
        for edit_point in &edit_points {
            let made = self.made(*edit_point, kind, at);
            match grouped.iter_mut().find(|(group, _)| *group == made) {
                Some((_, alike)) => alike.push(*edit_point),
                None => grouped.push((made, vec![*edit_point])),
            }
        }
        let edits = grouped
            .into_iter()
            .map(|(transition, edit_points)| Edit::Transitions {
                edit_points,
                transition: Some(transition),
            })
            .collect();
        self.commit_edits(edits);
        if let [edit_point] = edit_points.as_slice()
            && let Some(edit_point) = self.project().canonical_edit_point(*edit_point)
        {
            self.session.selected = Arc::default();
            self.session.edit_point = Some(edit_point);
            self.side = Side::Properties;
        }
    }

    fn made(
        &self,
        edit_point: EditPoint,
        kind: TransitionKind,
        at: Option<TransitionAlignment>,
    ) -> Transition {
        let project = self.project();
        let made = match project.transition(edit_point) {
            Some(current) => current.with_kind(kind),
            None => Transition::new(kind, project.rate()),
        };
        match at {
            Some(at) => made.with_alignment(at),
            None => made,
        }
    }

    pub(crate) fn retune_transition(&mut self, change: impl FnOnce(&mut Transition)) {
        self.live.settle();
        let Some((edit_point, transition)) = self.held_transition() else {
            skip_view();
            return;
        };
        let mut changed = transition;
        change(&mut changed);
        if changed == transition {
            skip_view();
            return;
        }
        self.edit(Edit::Transitions {
            edit_points: vec![edit_point],
            transition: Some(changed),
        });
    }

    pub(crate) fn preview_transition(&mut self, change: impl FnOnce(&mut Transition)) {
        if let Some((edit_point, mut changed)) = self.held_transition() {
            change(&mut changed);
            self.preview_edit(&Edit::Transitions {
                edit_points: vec![edit_point],
                transition: Some(changed),
            });
        }
        skip_view();
    }

    pub(crate) fn remove_transition(&mut self) {
        let Some((edit_point, _)) = self.held_transition() else {
            skip_view();
            return;
        };
        self.end_transition_preview();
        self.edit(Edit::Transitions {
            edit_points: vec![edit_point],
            transition: None,
        });
        self.side = Side::Transitions;
    }

    pub(crate) fn has_removable_transition(&self) -> bool {
        self.session.selected.is_empty() && self.held_transition().is_some()
    }

    pub(crate) fn open_transition_easing(&mut self, anchor: Anchor) {
        match self.held_transition() {
            Some((edit_point, _)) => {
                self.session.popup = Some(Popup::Easing(Picking::transition(edit_point, anchor)))
            }
            None => skip_view(),
        }
    }

    pub(crate) fn ease_transition(&mut self, easing: Easing) {
        self.retune_transition(|transition| transition.easing = easing);
    }

    pub(crate) fn start_transition_preview(&mut self, kind: Option<TransitionKind>) {
        skip_view();
        let (Some(kind), Some(edit_point)) = (kind, self.session.edit_point) else {
            self.end_transition_preview();
            return;
        };
        let edit = Edit::Transitions {
            edit_points: vec![edit_point],
            transition: Some(self.made(edit_point, kind, None)),
        };
        let Ok((previewed, _)) = self.project().apply(&edit) else {
            return;
        };
        let Some(span) = previewed
            .transition_span(edit_point)
            .map(|transition_span| transition_span.span)
        else {
            return;
        };
        self.live.rework(self.document.project(), previewed);
        let lead = self.project().rate().frames_in(LEAD);
        let span = (span.start - lead).max(0)..span.end + lead;
        let home = self
            .session
            .transition_preview
            .as_ref()
            .map_or_else(|| self.transport.frame(), |preview| preview.home);
        self.session.transition_preview = Some(TransitionPreview {
            span: span.clone(),
            home,
        });
        self.playback.halt();
        self.transport.stop();
        self.transport.seek(span.start);
        self.transport.play(1.0);
        self.keep_ticking();
    }

    pub(crate) fn end_transition_preview(&mut self) {
        let Some(transition_preview) = self.session.transition_preview.take() else {
            return;
        };
        self.live.settle();
        self.transport.halt_at(transition_preview.home);
        self.playback.halt();
    }
}
