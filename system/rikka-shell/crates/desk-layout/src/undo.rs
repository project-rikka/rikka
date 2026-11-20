const KEPT: usize = 200;

#[derive(Clone, Debug)]
pub struct Undo<T> {
    past: Vec<T>,
    future: Vec<T>,
}

impl<T> Undo<T> {
    #[must_use]
    pub fn new() -> Undo<T> {
        Undo {
            past: Vec::new(),
            future: Vec::new(),
        }
    }

    pub fn record(&mut self, before: T) {
        self.future.clear();
        self.past.push(before);
        if self.past.len() > KEPT {
            self.past.remove(0);
        }
    }

    pub fn undo(&mut self, now: T) -> Option<T> {
        let earlier = self.past.pop()?;
        self.future.push(now);
        Some(earlier)
    }

    pub fn redo(&mut self, now: T) -> Option<T> {
        let later = self.future.pop()?;
        self.past.push(now);
        Some(later)
    }

    #[must_use]
    pub fn earlier(&self) -> Option<&T> {
        self.past.last()
    }

    #[must_use]
    pub fn later(&self) -> Option<&T> {
        self.future.last()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn forget(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

impl<T> Default for Undo<T> {
    fn default() -> Undo<T> {
        Undo::new()
    }
}
