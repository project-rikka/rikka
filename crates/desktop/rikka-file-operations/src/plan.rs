#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub items: u64,
    pub bytes: u64,
}

impl Plan {
    pub fn add(&mut self, other: Self) {
        self.items += other.items;
        self.bytes += other.bytes;
    }
}
