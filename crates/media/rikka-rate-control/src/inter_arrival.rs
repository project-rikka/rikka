use std::time::{Duration, Instant};

const GROUP_SPAN: Duration = Duration::from_millis(5);
const BURST_GAP_US: i64 = 5_000;
const LONGEST_BURST_US: i64 = 100_000;
const LONGEST_JUMP_US: i64 = 3_000_000;

#[derive(Clone, Copy, Debug)]
struct Group {
    first_send: Instant,
    last_send: Instant,
    first_arrival: i64,
    last_arrival: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Delta {
    pub(crate) send_ms: f64,
    pub(crate) arrival_ms: f64,
    pub(crate) arrival_us: i64,
}

#[derive(Debug, Default)]
pub(crate) struct InterArrival {
    current: Option<Group>,
    previous: Option<Group>,
}

impl InterArrival {
    pub(crate) fn add(&mut self, send: Instant, arrival: i64) -> Option<Delta> {
        let Some(current) = &mut self.current else {
            self.current = Some(Group::starting(send, arrival));
            return None;
        };
        if send < current.first_send {
            return None;
        }
        let inside = send.saturating_duration_since(current.first_send) <= GROUP_SPAN;
        let arrival_gap = arrival - current.last_arrival;
        let send_gap = send
            .saturating_duration_since(current.last_send)
            .as_micros() as i64;
        let burst = arrival_gap - send_gap < 0
            && arrival_gap <= BURST_GAP_US
            && arrival - current.first_arrival < LONGEST_BURST_US;
        if inside || burst {
            current.last_send = current.last_send.max(send);
            current.last_arrival = current.last_arrival.max(arrival);
            return None;
        }
        let finished = *current;
        let delta = self.previous.and_then(|previous| {
            let send_ms = finished
                .last_send
                .saturating_duration_since(previous.last_send)
                .as_secs_f64()
                * 1000.0;
            let arrival_gap = finished.last_arrival - previous.last_arrival;
            if !(0..=LONGEST_JUMP_US).contains(&arrival_gap) {
                return None;
            }
            Some(Delta {
                send_ms,
                arrival_ms: arrival_gap as f64 / 1000.0,
                arrival_us: finished.last_arrival,
            })
        });
        if self.previous.is_some() && delta.is_none() {
            self.previous = None;
        } else {
            self.previous = Some(finished);
        }
        self.current = Some(Group::starting(send, arrival));
        delta
    }
}

impl Group {
    fn starting(send: Instant, arrival: i64) -> Group {
        Group {
            first_send: send,
            last_send: send,
            first_arrival: arrival,
            last_arrival: arrival,
        }
    }
}
