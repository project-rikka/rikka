use crate::change::Change;

const DEPTH: usize = 256;

pub trait Undoable: Clone {
    fn is_empty(&self) -> bool;
}

impl Undoable for Change {
    fn is_empty(&self) -> bool {
        Change::is_empty(self)
    }
}

#[derive(Debug)]
struct Step<S, C> {
    group: Option<u64>,
    gesture: Option<u64>,
    edits: Vec<(S, C)>,
}

#[derive(Debug)]
pub struct History<S, C = Change> {
    done: Vec<Step<S, C>>,
    undone: Vec<Step<S, C>>,
}

impl<S, C> Default for History<S, C> {
    fn default() -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
        }
    }
}

impl<S: Copy + PartialEq, C: Undoable> History<S, C> {
    pub fn record(&mut self, subject: S, edit: C) {
        self.record_in(None, subject, edit);
    }

    pub fn record_in(&mut self, group: Option<u64>, subject: S, edit: C) {
        if edit.is_empty() {
            return;
        }
        self.undone.clear();
        if let Some(step) = self.done.last_mut()
            && group.is_some()
            && step.group == group
        {
            step.edits.push((subject, edit));
            return;
        }
        self.push(Step {
            group,
            gesture: None,
            edits: vec![(subject, edit)],
        });
    }

    pub fn record_gesture(
        &mut self,
        gesture: u64,
        subject: S,
        edit: C,
        merge: impl FnOnce(&mut C, C),
    ) {
        if let Some(step) = self.done.last_mut()
            && step.gesture == Some(gesture)
            && let [(held, last)] = step.edits.as_mut_slice()
            && *held == subject
        {
            self.undone.clear();
            merge(last, edit);
            if last.is_empty() {
                self.done.pop();
            }
            return;
        }
        if edit.is_empty() {
            return;
        }
        self.undone.clear();
        self.push(Step {
            group: None,
            gesture: Some(gesture),
            edits: vec![(subject, edit)],
        });
    }

    pub fn seal(&mut self) {
        if let Some(step) = self.done.last_mut() {
            step.gesture = None;
        }
    }

    fn push(&mut self, step: Step<S, C>) {
        self.done.push(step);
        if self.done.len() > DEPTH {
            self.done.remove(0);
        }
    }

    pub fn undo(&mut self) -> Option<Vec<(S, C)>> {
        let mut step = self.done.pop()?;
        step.gesture = None;
        let edits = step.edits.clone();
        self.undone.push(step);
        Some(edits)
    }

    pub fn redo(&mut self) -> Option<Vec<(S, C)>> {
        let step = self.undone.pop()?;
        let edits = step.edits.clone();
        self.done.push(step);
        Some(edits)
    }

    pub fn forget(&mut self, subject: S) {
        for steps in [&mut self.done, &mut self.undone] {
            for step in steps.iter_mut() {
                step.edits.retain(|(owner, _)| *owner != subject);
            }
            steps.retain(|step| !step.edits.is_empty());
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
}
