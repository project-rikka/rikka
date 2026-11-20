#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Depth(i32);

impl Depth {
    pub(crate) const fn above(floor: i32) -> Self {
        Self(floor)
    }

    pub(crate) fn next(&mut self) -> i32 {
        self.0 += 1;
        self.0
    }

    pub(crate) fn reserve(&mut self, count: usize) -> i32 {
        let floor = self.0;
        self.0 += i32::try_from(count).unwrap_or(i32::MAX / 2);
        floor
    }
}
