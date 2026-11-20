use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;

use crate::scene::NodeId;

const SETTLE_FRAMES: u16 = 4;

const ZOOM_SETTLE_FRAMES: u16 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Moving,
    Converting { patient: bool },
    Settled,
}

#[derive(Clone, Copy, Debug)]
struct Zoom {
    scale: f32,
    still: u16,
    settled: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Zooms {
    scales: FxHashMap<NodeId, Zoom>,
    settling: Vec<NodeId>,
    due: Vec<NodeId>,
}

impl Zooms {
    pub(crate) fn phase(&mut self, id: NodeId, scale: f32) -> Phase {
        let zoom = match self.scales.entry(id) {
            Entry::Vacant(vacant) => {
                vacant.insert(Zoom {
                    scale,
                    still: SETTLE_FRAMES,
                    settled: true,
                });
                return Phase::Settled;
            }
            Entry::Occupied(occupied) => occupied.into_mut(),
        };
        if zoom.scale != scale {
            if zoom.settled {
                self.settling.push(id);
            }
            *zoom = Zoom {
                scale,
                still: 0,
                settled: false,
            };
        }
        match (zoom.settled, zoom.still) {
            (true, _) => Phase::Settled,
            (false, still) if still < SETTLE_FRAMES => Phase::Moving,
            (false, still) => Phase::Converting {
                patient: still < SETTLE_FRAMES + ZOOM_SETTLE_FRAMES,
            },
        }
    }

    pub(crate) fn is_settled(&self, id: NodeId, scale: f32) -> bool {
        self.scales
            .get(&id)
            .is_none_or(|zoom| zoom.settled && zoom.scale == scale)
    }

    pub(crate) fn converted(&mut self, id: NodeId) {
        if let Some(zoom) = self.scales.get_mut(&id) {
            zoom.settled = true;
        }
        if let Some(at) = self.settling.iter().position(|settling| *settling == id) {
            self.settling.swap_remove(at);
        }
    }

    pub(crate) fn tick(&mut self) -> &[NodeId] {
        let Self {
            scales,
            settling,
            due,
        } = self;
        due.clear();
        settling.retain(|id| {
            let Some(zoom) = scales.get_mut(id).filter(|zoom| !zoom.settled) else {
                return false;
            };
            zoom.still = zoom.still.saturating_add(1);
            if zoom.still >= SETTLE_FRAMES {
                due.push(*id);
            }
            if zoom.still > SETTLE_FRAMES + ZOOM_SETTLE_FRAMES {
                zoom.settled = true;
                return false;
            }
            true
        });
        due
    }

    pub(crate) fn settling(&self) -> &[NodeId] {
        &self.settling
    }

    pub(crate) fn forget(&mut self, removed: &[NodeId]) {
        for id in removed {
            if self.scales.remove(id).is_some()
                && let Some(at) = self.settling.iter().position(|settling| settling == id)
            {
                self.settling.swap_remove(at);
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.scales.clear();
        self.settling.clear();
    }
}
