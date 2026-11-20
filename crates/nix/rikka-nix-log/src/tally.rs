#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub done: u64,
    pub expected: u64,
    pub running: u64,
    pub failed: u64,
}

impl Count {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.done == 0 && self.expected == 0 && self.running == 0 && self.failed == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub builds: Count,
    pub paths: Count,
    pub copied: Count,
    pub downloaded: Count,
}

impl Tally {
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        let expected = self.builds.expected + self.paths.expected;
        if expected == 0 {
            return None;
        }
        let done = (self.builds.done + self.paths.done).min(expected);
        Some((done as f64 / expected as f64) as f32)
    }
}
