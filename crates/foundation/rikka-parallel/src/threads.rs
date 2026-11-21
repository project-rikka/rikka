use std::num::NonZeroUsize;
use std::thread;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Threads(NonZeroUsize);

impl Threads {
    pub const ONE: Threads = Threads(NonZeroUsize::MIN);

    #[must_use]
    pub const fn new(count: usize) -> Threads {
        match NonZeroUsize::new(count) {
            Some(count) => Threads(count),
            None => Threads::ONE,
        }
    }

    #[must_use]
    pub fn available() -> Threads {
        Threads(thread::available_parallelism().unwrap_or(NonZeroUsize::MIN))
    }

    #[must_use]
    pub fn or_available(count: usize) -> Threads {
        match NonZeroUsize::new(count) {
            Some(count) => Threads(count),
            None => Threads::available(),
        }
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl Default for Threads {
    fn default() -> Threads {
        Threads::available()
    }
}

impl From<NonZeroUsize> for Threads {
    fn from(count: NonZeroUsize) -> Threads {
        Threads(count)
    }
}

impl From<Threads> for NonZeroUsize {
    fn from(threads: Threads) -> NonZeroUsize {
        threads.0
    }
}
