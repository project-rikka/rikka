use super::dawg::Dawg;
use crate::error::{Error, Result};

pub(crate) const LABEL_BITS: u32 = 8;
pub(crate) const ACCEPTING: u32 = 1 << LABEL_BITS;
pub(crate) const TARGET_SHIFT: u32 = LABEL_BITS + 1;

#[derive(Debug)]
pub(crate) struct Packed {
    pub(crate) root: u32,
    pub(crate) slots: Vec<u32>,
}

struct Placement {
    occupied: Vec<bool>,
    used_bases: Vec<bool>,
    first_free: usize,
}

impl Placement {
    fn fits(&self, base: usize, labels: &[u8]) -> bool {
        !self.used_bases.get(base).copied().unwrap_or(false)
            && labels.iter().all(|&label| {
                !self
                    .occupied
                    .get(base + usize::from(label))
                    .copied()
                    .unwrap_or(false)
            })
    }

    fn place(&mut self, labels: &[u8]) -> usize {
        let lowest = usize::from(labels[0]);
        let mut free = self.first_free;
        let base = loop {
            if free > lowest && self.fits(free - lowest, labels) {
                break free - lowest;
            }
            free += 1;
            while self.occupied.get(free).copied().unwrap_or(false) {
                free += 1;
            }
        };
        let end = base + usize::from(labels[labels.len() - 1]) + 1;
        if self.occupied.len() < end {
            self.occupied.resize(end, false);
        }
        if self.used_bases.len() <= base {
            self.used_bases.resize(base + 1, false);
        }
        self.used_bases[base] = true;
        for &label in labels {
            self.occupied[base + usize::from(label)] = true;
        }
        while self.occupied.get(self.first_free).copied().unwrap_or(false) {
            self.first_free += 1;
        }
        base
    }
}

pub(crate) fn pack(dawg: &Dawg<u8, bool>) -> Result<Packed> {
    let mut order = dawg.breadth_first();
    order.sort_by_key(|&state| std::cmp::Reverse(dawg.states[state].edges.len()));
    let mut bases = vec![0usize; dawg.states.len()];
    let mut placement = Placement {
        occupied: Vec::new(),
        used_bases: vec![true],
        first_free: 1,
    };
    for &state in &order {
        let labels: Vec<u8> = dawg.states[state]
            .edges
            .iter()
            .map(|&(label, _)| label)
            .collect();
        if labels.is_empty() {
            continue;
        }
        bases[state] = placement.place(&labels);
    }
    let limit = 1usize << (u32::BITS - TARGET_SHIFT);
    if placement.occupied.len() >= limit {
        return Err(Error::Data(format!(
            "a dictionary needs {} slots",
            placement.occupied.len()
        )));
    }
    let mut slots = vec![0u32; placement.occupied.len()];
    for &state in &order {
        let base = bases[state];
        for &(label, target) in &dawg.states[state].edges {
            let accepting = if dawg.states[target].accept {
                ACCEPTING
            } else {
                0
            };
            slots[base + usize::from(label)] =
                u32::from(label) | accepting | (bases[target] as u32) << TARGET_SHIFT;
        }
    }
    Ok(Packed {
        root: bases[dawg.root] as u32,
        slots,
    })
}
