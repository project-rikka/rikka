use std::ops::{Add, AddAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: u64,
}

impl Usage {
    #[must_use]
    pub fn of_prompt(prompt: u64, output: u64, cache_read: u64, cache_write: u64) -> Usage {
        Usage {
            input: prompt.saturating_sub(cache_read + cache_write),
            output,
            cache_read,
            cache_write,
            reasoning: 0,
        }
    }

    #[must_use]
    pub fn prompt(self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    #[must_use]
    pub fn total(self) -> u64 {
        self.prompt() + self.output
    }
}

impl Add for Usage {
    type Output = Usage;

    fn add(self, other: Usage) -> Usage {
        Usage {
            input: self.input + other.input,
            output: self.output + other.output,
            cache_read: self.cache_read + other.cache_read,
            cache_write: self.cache_write + other.cache_write,
            reasoning: self.reasoning + other.reasoning,
        }
    }
}

impl AddAssign for Usage {
    fn add_assign(&mut self, other: Usage) {
        *self = *self + other;
    }
}

impl std::iter::Sum for Usage {
    fn sum<I: Iterator<Item = Usage>>(iterator: I) -> Usage {
        iterator.fold(Usage::default(), Add::add)
    }
}
