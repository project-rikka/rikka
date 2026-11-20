use std::collections::VecDeque;

const ACKED_WINDOW_US: i64 = 500_000;
const SHORTEST_SPAN_US: i64 = 100_000;

#[derive(Debug, Default)]
pub(crate) struct Acked {
    samples: VecDeque<(i64, usize)>,
}

impl Acked {
    pub(crate) fn add(&mut self, arrival_us: i64, bytes: usize) {
        self.samples.push_back((arrival_us, bytes));
        while self
            .samples
            .front()
            .is_some_and(|(oldest, _)| arrival_us - oldest > ACKED_WINDOW_US)
        {
            self.samples.pop_front();
        }
    }

    pub(crate) fn rate(&self) -> Option<f64> {
        let (first, _) = self.samples.front()?;
        let (last, _) = self.samples.back()?;
        let span = last - first;
        if span < SHORTEST_SPAN_US {
            return None;
        }
        let bytes: usize = self.samples.iter().skip(1).map(|(_, bytes)| bytes).sum();
        Some(bytes as f64 * 8.0 * 1_000_000.0 / span as f64)
    }
}
