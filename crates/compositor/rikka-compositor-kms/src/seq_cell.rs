use std::hint;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};

#[derive(Debug)]
pub(crate) struct SeqCell<const N: usize> {
    version: AtomicU32,
    words: [AtomicU64; N],
}

impl<const N: usize> SeqCell<N> {
    pub(crate) fn new(words: [u64; N]) -> Self {
        Self {
            version: AtomicU32::new(0),
            words: words.map(AtomicU64::new),
        }
    }

    pub(crate) fn load(&self) -> [u64; N] {
        loop {
            let before = self.version.load(Ordering::Acquire);
            if before & 1 == 1 {
                hint::spin_loop();
                continue;
            }
            let words = std::array::from_fn(|index| self.words[index].load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if self.version.load(Ordering::Relaxed) == before {
                return words;
            }
        }
    }

    pub(crate) fn store(&self, words: [u64; N]) {
        let version = self.version.load(Ordering::Relaxed);
        self.version
            .store(version.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for (slot, word) in self.words.iter().zip(words) {
            slot.store(word, Ordering::Relaxed);
        }
        self.version
            .store(version.wrapping_add(2), Ordering::Release);
    }
}
