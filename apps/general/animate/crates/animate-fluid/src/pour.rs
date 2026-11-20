use std::ops::RangeInclusive;

use rikka_mmd::FRAMES_PER_SECOND;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pour {
    start: u32,
    length: u32,
}

impl Pour {
    pub const LENGTH: u32 = 2 * FRAMES_PER_SECOND as u32;
    pub const STARTS: RangeInclusive<u32> = 0..=1_000_000;
    pub const LENGTHS: RangeInclusive<u32> = 1..=600 * FRAMES_PER_SECOND as u32;

    #[must_use]
    pub fn new(start: u32, length: u32) -> Self {
        Self {
            start: start.min(*Self::STARTS.end()),
            length: length.clamp(*Self::LENGTHS.start(), *Self::LENGTHS.end()),
        }
    }

    #[must_use]
    pub fn checked(start: u32, length: u32) -> Option<Self> {
        (Self::STARTS.contains(&start) && Self::LENGTHS.contains(&length))
            .then_some(Self { start, length })
    }

    #[must_use]
    pub fn from_start(start: u32) -> Self {
        Self::new(start, Self::LENGTH)
    }

    #[must_use]
    pub fn start(self) -> u32 {
        self.start
    }

    #[must_use]
    pub fn length(self) -> u32 {
        self.length
    }

    #[must_use]
    pub fn end(self) -> u32 {
        self.start + self.length
    }

    #[must_use]
    pub fn seconds(self) -> f32 {
        self.length as f32 / FRAMES_PER_SECOND
    }

    #[must_use]
    pub fn holds(self, frame: f32) -> bool {
        self.start as f32 <= frame && frame < self.end() as f32
    }

    #[must_use]
    pub fn can_start_at(self, frame: u32) -> bool {
        frame != self.start
    }

    #[must_use]
    pub fn can_end_at(self, frame: u32) -> bool {
        frame > self.start && frame != self.end()
    }

    #[must_use]
    pub fn moved_to(self, start: u32) -> Self {
        Self::new(start, self.length)
    }

    #[must_use]
    pub fn lasting(self, length: u32) -> Self {
        Self::new(self.start, length)
    }

    #[must_use]
    pub fn lasting_seconds(self, seconds: f32) -> Self {
        self.lasting((seconds.max(0.0) * FRAMES_PER_SECOND).round() as u32)
    }

    #[must_use]
    pub fn starting_at(self, start: u32) -> Self {
        let start = start.min(self.end() - 1);
        Self::new(start, self.end() - start)
    }

    #[must_use]
    pub fn ending_at(self, end: u32) -> Self {
        self.lasting(end.saturating_sub(self.start))
    }
}

impl Default for Pour {
    fn default() -> Self {
        Self::from_start(0)
    }
}
