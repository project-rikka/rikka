use animate_cast::{FigureId, Subject};
use animate_hold::{Hold, HoldId, Holds};
use animate_inspector::Listed;
use animate_inspector::PoseEditing;
use animate_inspector::{HoldDial, HoldEditing};
use animate_motion::Take;
use animate_sheet::HoldSpan;

use crate::animate::Animate;

impl Animate {
    pub(crate) fn grasp(&mut self, hold: Hold) {
        let Some(Subject::Figure(id)) = self.subject else {
            return;
        };
        self.in_undo_group(|animate| {
            let edited = animate.unregistered_parts();
            if !edited.is_empty() {
                animate.register_edited(edited);
            }
            animate.lay_hold(id, hold);
        });
    }

    pub(crate) fn lay_hold(&mut self, id: FigureId, hold: Hold) {
        self.change_holds(id, |holds| {
            holds.lay(hold);
        });
    }

    pub(crate) fn can_release_hold(&self) -> bool {
        let Some(Subject::Figure(id)) = self.subject else {
            return false;
        };
        let frame = self.transport.frame();
        self.take_of(id).holds.holding(frame).next().is_some()
    }

    pub(crate) fn release_holds(&mut self) {
        let Some(Subject::Figure(id)) = self.subject else {
            return;
        };
        let frame = self.transport.frame();
        self.change_holds(id, |holds| {
            let active: Vec<HoldId> = holds.holding(frame).map(|(hold, _)| hold).collect();
            for hold in active {
                holds.release(hold, frame);
            }
        });
    }

    pub(crate) fn retime_chosen_hold(&mut self, span: HoldSpan) {
        let Some(Subject::Figure(id)) = self.subject else {
            return;
        };
        self.change_holds(id, |holds| {
            holds.retime(span.id, span.span.start, span.span.end)
        });
        self.timeline.follow_hold(span.id);
    }

    pub(crate) fn chosen_hold(&self) -> Option<(FigureId, HoldId)> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        let hold = self.timeline.chosen_hold()?;
        self.take_of(id).holds.get(hold).map(|_| (id, hold))
    }

    fn change_holds(&mut self, id: FigureId, edit: impl FnOnce(&mut Holds)) {
        self.update::<Take>(Subject::Figure(id), |take| edit(&mut take.holds));
    }
}

impl HoldEditing for Animate {
    fn apply_hold(&mut self, hold: HoldId, dial: HoldDial, value: f32) {
        self.controls.hold_draft = None;
        let Some(Subject::Figure(id)) = self.subject else {
            return;
        };
        self.change_holds(id, |holds| match dial {
            HoldDial::Reach => holds.set_reach(hold, value),
            HoldDial::Ease => holds.set_ease(hold, value.round().max(0.0) as u32),
        });
    }
    fn draft_hold(&mut self, hold: HoldId, dial: HoldDial, value: f32) {
        self.controls.hold_draft = Some((hold, dial, value));
    }
    fn drop_hold(&mut self, id: FigureId, hold: HoldId) {
        self.change_holds(id, |holds| {
            holds.remove(hold);
        });
    }
    fn hold_dial(&self, listed: &Listed, dial: HoldDial) -> f32 {
        match self.controls.hold_draft {
            Some((id, drafted, value)) if id == listed.id && drafted == dial => value,
            _ => match dial {
                HoldDial::Reach => listed.hold.target.reach,
                HoldDial::Ease => listed.hold.ease as f32,
            },
        }
    }
    fn listed_holds(&self) -> Vec<Listed> {
        let Some(Subject::Figure(id)) = self.subject else {
            return Vec::new();
        };
        let Some(figure) = self.cast.figure(id) else {
            return Vec::new();
        };
        let frame = self.transport.frame();
        let chosen = self.timeline.chosen_hold();
        figure
            .take
            .holds
            .entries()
            .filter(|(id, hold)| hold.holds_at(frame) || chosen == Some(*id))
            .map(|(id, hold)| Listed {
                id,
                hold: hold.clone(),
                holding: hold.holds_at(frame),
                owner: hold
                    .target
                    .figure
                    .clone()
                    .unwrap_or_else(|| figure.name.clone()),
            })
            .collect()
    }
    fn release_hold(&mut self, id: FigureId, hold: HoldId) {
        let frame = self.transport.frame();
        self.change_holds(id, |holds| holds.release(hold, frame));
    }
    fn subject_figure(&self) -> Option<FigureId> {
        match self.subject {
            Some(Subject::Figure(id)) => Some(id),
            _ => None,
        }
    }
}
