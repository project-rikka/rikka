#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    View,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Done,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Transfer {
    pub id: u64,
    pub name: String,
    pub direction: Direction,
    pub done: u64,
    pub total: u64,
    pub state: State,
}

impl Transfer {
    #[must_use]
    pub fn share(&self) -> Option<f32> {
        (self.total > 0).then(|| {
            let done = self.done.min(self.total) as f64;
            (done / self.total as f64) as f32
        })
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == State::Running
    }
}

#[derive(Debug, Default)]
pub struct Transfers {
    transfers: Vec<Transfer>,
    next: u64,
}

impl Transfers {
    #[must_use]
    pub fn new() -> Transfers {
        Transfers::default()
    }

    pub fn start(&mut self, name: impl Into<String>, direction: Direction, total: u64) -> u64 {
        self.next += 1;
        self.transfers.push(Transfer {
            id: self.next,
            name: name.into(),
            direction,
            done: 0,
            total,
            state: State::Running,
        });
        self.next
    }

    pub fn progress(&mut self, id: u64, done: u64) {
        if let Some(transfer) = self.get_mut(id) {
            transfer.done = done;
        }
    }

    pub fn finish(&mut self, id: u64) {
        if let Some(transfer) = self.get_mut(id) {
            transfer.done = transfer.total;
            transfer.state = State::Done;
        }
    }

    pub fn failed(&mut self, id: u64, reason: impl Into<String>) {
        if let Some(transfer) = self.get_mut(id) {
            transfer.state = State::Failed(reason.into());
        }
    }

    pub fn forget(&mut self, id: u64) {
        self.transfers.retain(|transfer| transfer.id != id);
    }

    #[must_use]
    pub fn all(&self) -> &[Transfer] {
        &self.transfers
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.transfers.is_empty()
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Transfer> {
        self.transfers.iter_mut().find(|transfer| transfer.id == id)
    }
}
