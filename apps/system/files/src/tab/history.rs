use files_core::Location;

#[derive(Debug, Default)]
pub(crate) struct History {
    back: Vec<Location>,
    forward: Vec<Location>,
}

impl History {
    pub(crate) fn visit(&mut self, from: Location) {
        self.back.push(from);
        self.forward.clear();
    }

    pub(crate) fn back(&mut self, here: Location) -> Option<Location> {
        let previous = self.back.pop()?;
        self.forward.push(here);
        Some(previous)
    }

    pub(crate) fn forward(&mut self, here: Location) -> Option<Location> {
        let next = self.forward.pop()?;
        self.back.push(here);
        Some(next)
    }

    pub(crate) fn at(&self) -> usize {
        self.back.len()
    }

    pub(crate) fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}
