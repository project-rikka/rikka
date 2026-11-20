#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BufferId {
    Sole,
    External(u64),
}

#[derive(Debug, Default)]
pub struct Identities {
    last: u64,
}

impl Identities {
    pub fn target(&mut self) -> TargetId {
        TargetId(self.next())
    }

    pub fn external(&mut self) -> BufferId {
        BufferId::External(self.next())
    }

    fn next(&mut self) -> u64 {
        self.last += 1;
        self.last
    }
}
