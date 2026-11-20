const SMOKE_STEPS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Full,
    Smoke,
}

impl Size {
    pub fn from_args() -> Self {
        match std::env::args().any(|argument| argument == "--bench") {
            true => Self::Full,
            false => Self::Smoke,
        }
    }

    pub(crate) fn pick<T>(self, full: T, smoke: T) -> T {
        match self {
            Self::Full => full,
            Self::Smoke => smoke,
        }
    }

    pub(crate) fn steps(self, steps: u32) -> u32 {
        self.pick(steps, steps.min(SMOKE_STEPS))
    }

    pub fn worker_counts(self) -> Vec<usize> {
        let available = std::thread::available_parallelism().map_or(1, usize::from);
        let limit = self.pick(available, available.min(2));
        let mut counts: Vec<usize> = std::iter::successors(Some(1), |count| Some(count * 2))
            .take_while(|&count| count <= limit)
            .collect();
        if counts.last() != Some(&limit) {
            counts.push(limit);
        }
        counts
    }
}
