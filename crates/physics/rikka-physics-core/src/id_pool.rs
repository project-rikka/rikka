#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdPool {
    free: Vec<u32>,
    next: u32,
}

impl IdPool {
    pub const fn new() -> Self {
        Self {
            free: Vec::new(),
            next: 0,
        }
    }

    #[inline]
    pub fn alloc(&mut self) -> u32 {
        if let Some(index) = self.free.pop() {
            return index;
        }
        let index = self.next;
        self.next = index.checked_add(1).expect("id pool exhausted u32 indices");
        index
    }

    #[inline]
    pub fn free(&mut self, index: u32) {
        debug_assert!(index < self.next, "freed index {index} was never allocated");
        self.free.push(index);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.next as usize - self.free.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn capacity(&self) -> u32 {
        self.next
    }

    pub fn clear(&mut self) {
        self.free.clear();
        self.next = 0;
    }
}
