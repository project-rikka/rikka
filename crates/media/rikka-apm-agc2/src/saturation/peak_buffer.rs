const CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PeakBuffer {
    values: [f32; CAPACITY],
    next: usize,
    size: usize,
}

impl PeakBuffer {
    pub(crate) fn new() -> PeakBuffer {
        PeakBuffer {
            values: [0.0; CAPACITY],
            next: 0,
            size: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.next = 0;
        self.size = 0;
    }

    pub(crate) fn push(&mut self, value: f32) {
        self.values[self.next] = value;
        self.next = (self.next + 1) % CAPACITY;
        if self.size < CAPACITY {
            self.size += 1;
        }
    }

    pub(crate) fn front(&self) -> Option<f32> {
        if self.size == 0 {
            return None;
        }
        let index = if self.size == CAPACITY { self.next } else { 0 };
        Some(self.values[index])
    }
}
