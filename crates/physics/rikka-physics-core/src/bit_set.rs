#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    pub const fn new() -> Self {
        Self { words: Vec::new() }
    }

    pub fn with_capacity(bits: usize) -> Self {
        Self {
            words: vec![0; bits.div_ceil(64)],
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.words.len() * 64
    }

    pub fn grow(&mut self, bits: usize) {
        let words = bits.div_ceil(64);
        if words > self.words.len() {
            self.words.resize(words, 0);
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize) {
        self.words[index / 64] |= 1 << (index % 64);
    }

    #[inline]
    pub fn set_growing(&mut self, index: usize) {
        self.grow(index + 1);
        self.set(index);
    }

    #[inline]
    pub fn clear(&mut self, index: usize) {
        if let Some(word) = self.words.get_mut(index / 64) {
            *word &= !(1 << (index % 64));
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> bool {
        self.words
            .get(index / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    pub fn clear_all(&mut self) {
        self.words.fill(0);
    }

    pub fn union_with(&mut self, other: &Self) {
        self.grow(other.capacity());
        for (word, &other) in self.words.iter_mut().zip(&other.words) {
            *word |= other;
        }
    }

    pub fn count(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&word| word == 0)
    }

    #[inline]
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    #[inline]
    pub fn iter(&self) -> Ones<'_> {
        Ones {
            words: &self.words,
            base: 0,
            current: self.words.first().copied().unwrap_or(0),
        }
    }
}

impl<'a> IntoIterator for &'a BitSet {
    type Item = usize;
    type IntoIter = Ones<'a>;

    fn into_iter(self) -> Ones<'a> {
        self.iter()
    }
}

#[derive(Clone, Debug)]
pub struct Ones<'a> {
    words: &'a [u64],
    base: usize,
    current: u64,
}

impl Iterator for Ones<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        while self.current == 0 {
            self.base += 1;
            self.current = *self.words.get(self.base)?;
        }
        let bit = self.current.trailing_zeros() as usize;
        self.current &= self.current - 1;
        Some(self.base * 64 + bit)
    }
}
