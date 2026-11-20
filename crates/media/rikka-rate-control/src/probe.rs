use std::collections::HashMap;
use std::time::Instant;

use crate::history::PacketResult;

pub(crate) const LEAST_PROBE_PACKETS: usize = 5;
const SATURATED: f64 = 0.9;
const SATURATED_SHARE: f64 = 0.95;
const SHORTEST_SPAN_US: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeCluster {
    pub id: u32,
    pub rate_bps: u64,
    pub packets: usize,
}

#[derive(Debug)]
struct Cluster {
    planned: usize,
    counted: usize,
    received: usize,
    first_send: Option<Instant>,
    last_send: Option<Instant>,
    last_send_bytes: usize,
    send_bytes: usize,
    first_arrival: Option<i64>,
    first_arrival_bytes: usize,
    last_arrival: Option<i64>,
    receive_bytes: usize,
}

#[derive(Debug, Default)]
pub(crate) struct ProbeResults {
    clusters: HashMap<u32, Cluster>,
}

impl ProbeResults {
    pub(crate) fn planned(&mut self, cluster: ProbeCluster) {
        self.clusters.insert(
            cluster.id,
            Cluster {
                planned: cluster.packets,
                counted: 0,
                received: 0,
                first_send: None,
                last_send: None,
                last_send_bytes: 0,
                send_bytes: 0,
                first_arrival: None,
                first_arrival_bytes: 0,
                last_arrival: None,
                receive_bytes: 0,
            },
        );
    }

    pub(crate) fn add(&mut self, results: &[PacketResult]) -> Option<f64> {
        let mut best: Option<f64> = None;
        for result in results {
            let Some(id) = result.sent.probe else {
                continue;
            };
            let Some(cluster) = self.clusters.get_mut(&id) else {
                continue;
            };
            cluster.counted += 1;
            cluster.send_bytes += result.sent.bytes;
            if cluster
                .first_send
                .is_none_or(|first| result.sent.at < first)
            {
                cluster.first_send = Some(result.sent.at);
            }
            if cluster.last_send.is_none_or(|last| result.sent.at >= last) {
                cluster.last_send = Some(result.sent.at);
                cluster.last_send_bytes = result.sent.bytes;
            }
            if let Some(arrival) = result.arrival_us {
                cluster.received += 1;
                cluster.receive_bytes += result.sent.bytes;
                if cluster.first_arrival.is_none_or(|first| arrival < first) {
                    cluster.first_arrival = Some(arrival);
                    cluster.first_arrival_bytes = result.sent.bytes;
                }
                if cluster.last_arrival.is_none_or(|last| arrival > last) {
                    cluster.last_arrival = Some(arrival);
                }
            }
            if cluster.counted < cluster.planned {
                continue;
            }
            let finished = self.clusters.remove(&id);
            if let Some(estimate) = finished.and_then(|cluster| cluster.estimate()) {
                best = Some(best.map_or(estimate, |best| best.max(estimate)));
            }
        }
        best
    }
}

impl Cluster {
    fn estimate(&self) -> Option<f64> {
        if self.received < LEAST_PROBE_PACKETS {
            return None;
        }
        let send_span = self
            .last_send?
            .saturating_duration_since(self.first_send?)
            .as_micros() as f64;
        let receive_span = (self.last_arrival? - self.first_arrival?) as f64;
        if send_span < SHORTEST_SPAN_US || receive_span < SHORTEST_SPAN_US {
            return None;
        }
        let send_rate = (self.send_bytes - self.last_send_bytes) as f64 * 8.0e6 / send_span;
        let receive_rate =
            (self.receive_bytes - self.first_arrival_bytes) as f64 * 8.0e6 / receive_span;
        Some(match receive_rate < SATURATED * send_rate {
            true => receive_rate * SATURATED_SHARE,
            false => send_rate.min(receive_rate),
        })
    }
}
