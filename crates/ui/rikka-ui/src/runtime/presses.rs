use std::time::{Duration, Instant};

use rikka_types::{Point, PointerButton};

const INTERVAL: Duration = Duration::from_millis(500);
const SLOP: f32 = 5.0;

#[derive(Debug, Default)]
pub(crate) struct Presses {
    last: Option<Stroke>,
    earlier: Vec<Received>,
    current: Vec<Received>,
}

#[derive(Debug, Clone, Copy)]
struct Stroke {
    button: PointerButton,
    at: Point,
    when: Instant,
}

#[derive(Debug, Clone, Copy)]
struct Received {
    node: u64,
    count: u32,
}

impl Presses {
    pub(crate) fn press(&mut self, button: PointerButton, at: Point, now: Instant) {
        let continues = self.last.is_some_and(|last| {
            last.button == button
                && now.saturating_duration_since(last.when) <= INTERVAL
                && at.distance(last.at) <= SLOP
        });
        self.last = Some(Stroke {
            button,
            at,
            when: now,
        });
        std::mem::swap(&mut self.earlier, &mut self.current);
        self.current.clear();
        if !continues {
            self.earlier.clear();
        }
    }

    pub(crate) fn receive(&mut self, node: u64) -> u32 {
        if let Some(received) = self.current.iter().find(|received| received.node == node) {
            return received.count;
        }
        let count = self
            .earlier
            .iter()
            .find(|received| received.node == node)
            .map_or(1, |received| received.count.saturating_add(1));
        self.current.push(Received { node, count });
        count
    }
}
