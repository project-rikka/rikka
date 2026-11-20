const DEFAULT_INLINE: usize = 64;

pub(crate) struct Stack<T: Copy + Default, const INLINE: usize = DEFAULT_INLINE> {
    inline: [T; INLINE],
    len: usize,
    spill: Vec<T>,
}

impl<T: Copy + Default> Stack<T> {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::empty()
    }
}

impl<T: Copy + Default, const INLINE: usize> Stack<T, INLINE> {
    #[inline]
    pub(crate) fn empty() -> Self {
        Self {
            inline: [T::default(); INLINE],
            len: 0,
            spill: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, value: T) {
        if self.len < INLINE {
            self.inline[self.len] = value;
            self.len += 1;
        } else {
            self.spill.push(value);
        }
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<T> {
        if let Some(value) = self.spill.pop() {
            return Some(value);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.inline[self.len])
    }
}
