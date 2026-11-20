use rikka_types::Point;

use crate::marks::mark::Mark;

#[derive(Clone, Debug)]
pub struct RemovedMark {
    pub index: usize,
    pub mark: Mark,
}

#[derive(Clone, Debug)]
enum Step {
    Draw(RemovedMark),
    Erase(Vec<RemovedMark>),
}

#[derive(Clone, Debug, Default)]
pub struct Sketch {
    marks: Vec<Mark>,
    done: Vec<Step>,
    undone: Vec<Step>,
    revision: u64,
}

impl Sketch {
    #[must_use]
    pub fn new() -> Sketch {
        Sketch::default()
    }

    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn push(&mut self, mark: Mark) {
        let index = self.marks.len();
        self.marks.push(mark.clone());
        self.done.push(Step::Draw(RemovedMark { index, mark }));
        self.undone.clear();
        self.bump_revision();
    }

    #[must_use]
    pub fn remove_at(&mut self, at: Point, reach: f32) -> Option<RemovedMark> {
        let index = self
            .marks
            .iter()
            .rposition(|mark| mark.touches(at, reach))?;
        let removed = RemovedMark {
            index,
            mark: self.marks.remove(index),
        };
        self.bump_revision();
        Some(removed)
    }

    pub fn commit_erase(&mut self, removed: Vec<RemovedMark>) {
        if removed.is_empty() {
            return;
        }
        self.done.push(Step::Erase(removed));
        self.undone.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(step) = self.done.pop() else {
            return false;
        };
        match &step {
            Step::Draw(removed) => self.remove_index(removed.index),
            Step::Erase(removed) => self.restore(removed),
        }
        self.undone.push(step);
        self.bump_revision();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(step) = self.undone.pop() else {
            return false;
        };
        match &step {
            Step::Draw(removed) => self.restore(std::slice::from_ref(removed)),
            Step::Erase(removed) => {
                for removed in removed.iter().rev() {
                    self.remove_index(removed.index);
                }
            }
        }
        self.done.push(step);
        self.bump_revision();
        true
    }

    fn remove_index(&mut self, index: usize) {
        if index < self.marks.len() {
            self.marks.remove(index);
        }
    }

    fn restore(&mut self, removed: &[RemovedMark]) {
        for removed in removed {
            let index = removed.index.min(self.marks.len());
            self.marks.insert(index, removed.mark.clone());
        }
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
