use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Configures<T> {
    pending: VecDeque<(u32, T)>,
}

impl<T: PartialEq> Configures<T> {
    #[must_use]
    pub fn new() -> Configures<T> {
        Configures {
            pending: VecDeque::new(),
        }
    }

    pub fn push(&mut self, serial: u32, shape: T) {
        self.pending.push_back((serial, shape));
    }

    pub fn ack_for_commit(&mut self, committed: &T) -> Option<u32> {
        let index = self
            .pending
            .iter()
            .rposition(|(_, shape)| shape == committed)?;
        let (serial, _) = self.pending.drain(..=index).next_back()?;
        Some(serial)
    }
}

impl<T: PartialEq> Default for Configures<T> {
    fn default() -> Configures<T> {
        Configures::new()
    }
}
