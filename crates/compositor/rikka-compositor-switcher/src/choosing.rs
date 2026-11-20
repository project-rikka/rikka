use rikka_session::switcher::Rows;
use rikka_types::Modifiers;

use crate::holding::Holding;
use crate::step::Step;

#[derive(Debug)]
pub struct Choosing<W> {
    windows: Vec<W>,
    selected: usize,
    holding: Holding,
    rows: Rows,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Finished<W> {
    pub chosen: Option<W>,
    pub lingering: Option<Holding>,
}

impl<W: Copy + Eq> Choosing<W> {
    pub fn begin(
        windows: Vec<W>,
        current: Option<W>,
        back: bool,
        pressed: Modifiers,
    ) -> Option<Self> {
        let last = windows.len().checked_sub(1)?;
        let selected = match (back, windows.first() == current.as_ref()) {
            (true, _) => last,
            (false, true) => 1.min(last),
            (false, false) => 0,
        };
        Some(Self {
            windows,
            selected,
            holding: Holding::of(pressed),
            rows: Rows::default(),
        })
    }

    pub fn windows(&self) -> &[W] {
        &self.windows
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn chosen(&self) -> Option<W> {
        self.windows.get(self.selected).copied()
    }

    pub fn holding(&self) -> Holding {
        self.holding
    }

    pub fn is_sticky(&self) -> bool {
        self.holding.is_sticky()
    }

    pub fn arrange(&mut self, rows: Rows) {
        self.rows = rows;
    }

    pub fn take(&mut self, step: Step) -> bool {
        let count = self.windows.len();
        if count == 0 {
            return false;
        }
        let target = match step {
            Step::Next => Some((self.selected + 1) % count),
            Step::Previous => Some((self.selected + count - 1) % count),
            Step::Up => self.rows.above(self.selected),
            Step::Down => self.rows.below(self.selected),
            Step::First => Some(0),
            Step::Last => Some(count - 1),
        };
        target.is_some_and(|index| self.select(index))
    }

    pub fn pick(&mut self, window: W) -> Option<bool> {
        let at = self
            .windows
            .iter()
            .position(|candidate| *candidate == window)?;
        Some(self.select(at))
    }

    pub fn lose(&mut self, window: W) -> bool {
        let Some(at) = self
            .windows
            .iter()
            .position(|candidate| *candidate == window)
        else {
            return false;
        };
        self.windows.remove(at);
        if at < self.selected || self.selected >= self.windows.len() {
            self.selected = self.selected.saturating_sub(1);
        }
        self.rows = Rows::default();
        true
    }

    pub fn modifiers_released(&self, pressed: Modifiers) -> bool {
        !self.holding.is_sticky() && !self.holding.held_by(pressed)
    }

    pub fn finish(self, choose: bool, pressed: Modifiers) -> Finished<W> {
        let lingering =
            (!self.holding.is_sticky() && self.holding.held_by(pressed)).then_some(self.holding);
        Finished {
            chosen: choose.then(|| self.chosen()).flatten(),
            lingering,
        }
    }

    fn select(&mut self, index: usize) -> bool {
        if index >= self.windows.len() || index == self.selected {
            return false;
        }
        self.selected = index;
        true
    }
}
