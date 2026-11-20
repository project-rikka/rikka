use std::cmp::Ordering;
use std::ops::Add;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score(f32);

impl Score {
    pub const NEUTRAL: Score = Score(0.0);

    #[must_use]
    pub const fn new(value: f32) -> Score {
        Score(value)
    }

    #[must_use]
    pub fn scaled(self, factor: f32) -> Score {
        Score(self.0 * factor)
    }

    #[must_use]
    pub fn offset(self, amount: f32) -> Score {
        Score(self.0 + amount)
    }
}

impl Add for Score {
    type Output = Score;

    fn add(self, other: Score) -> Score {
        Score(self.0 + other.0)
    }
}

impl Eq for Score {}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
