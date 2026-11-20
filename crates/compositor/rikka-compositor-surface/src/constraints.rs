use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Constraints {
    pub(crate) set_barrier: bool,
    pub(crate) wait_barrier: bool,
    pub(crate) target: Option<Duration>,
}

impl Constraints {
    pub(crate) fn absorb(&mut self, later: Constraints) {
        self.set_barrier |= later.set_barrier;
        self.wait_barrier |= later.wait_barrier;
        if later.target.is_some() {
            self.target = later.target;
        }
    }
}
