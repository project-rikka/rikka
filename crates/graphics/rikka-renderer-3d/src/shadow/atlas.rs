#[derive(Debug)]
pub(crate) struct AtlasAllocator {
    size: u32,
    free: Vec<Vec<(u32, u32)>>,
}

const FREE_CAPACITY: usize = 96;

impl AtlasAllocator {
    pub(crate) fn new(size: u32) -> Self {
        let levels = size.trailing_zeros() as usize + 1;
        let mut free: Vec<Vec<(u32, u32)>> = (0..levels)
            .map(|_| Vec::with_capacity(FREE_CAPACITY))
            .collect();
        free[0].push((0, 0));
        Self { size, free }
    }

    pub(crate) fn reset(&mut self) {
        for level in &mut self.free {
            level.clear();
        }
        self.free[0].push((0, 0));
    }

    fn level_of(&self, tile: u32) -> Option<usize> {
        if !tile.is_power_of_two() || tile > self.size {
            return None;
        }
        Some((self.size.trailing_zeros() - tile.trailing_zeros()) as usize)
    }

    fn split(&mut self, level: usize, origin: (u32, u32)) {
        let half = self.size >> (level + 1);
        let (x, y) = origin;
        self.free[level + 1].extend([(x + half, y), (x, y + half), (x + half, y + half)]);
    }

    pub(crate) fn allocate(&mut self, tile: u32) -> Option<(u32, u32)> {
        let target_level = self.level_of(tile)?;
        let found = (0..=target_level)
            .rev()
            .find(|&level| !self.free[level].is_empty())?;

        let origin = self.free[found]
            .pop()
            .expect("free list level is non-empty");
        for level in found..target_level {
            self.split(level, origin);
        }
        Some(origin)
    }

    pub(crate) fn reserve(&mut self, origin: (u32, u32), tile: u32) -> bool {
        let Some(target_level) = self.level_of(tile) else {
            return false;
        };
        for level in (0..=target_level).rev() {
            let side = self.size >> level;
            let block = (origin.0 / side * side, origin.1 / side * side);
            let Some(at) = self.free[level].iter().position(|&free| free == block) else {
                continue;
            };
            self.free[level].swap_remove(at);
            let mut current = block;
            for deeper in level..target_level {
                let half = self.size >> (deeper + 1);
                let child = (
                    current.0 + (origin.0 - current.0) / half * half,
                    current.1 + (origin.1 - current.1) / half * half,
                );
                for corner in [(0, 0), (half, 0), (0, half), (half, half)] {
                    let sibling = (current.0 + corner.0, current.1 + corner.1);
                    if sibling != child {
                        self.free[deeper + 1].push(sibling);
                    }
                }
                current = child;
            }
            return true;
        }
        false
    }
}
