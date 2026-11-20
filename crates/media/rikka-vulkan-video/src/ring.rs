use crate::error::{Error, Result};
use crate::syntax::{Plan, Reference};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    frame: u32,
    intra: bool,
}

#[derive(Debug)]
pub(crate) struct Ring {
    slots: Vec<Option<Held>>,
    depth: u32,
    next: u32,
    intra: Option<u32>,
    idr_id: u16,
}

impl Ring {
    pub(crate) fn new(depth: u8) -> Ring {
        Ring {
            slots: vec![None; usize::from(depth) + 1],
            depth: u32::from(depth),
            next: 0,
            intra: None,
            idr_id: 0,
        }
    }

    pub(crate) fn next_frame(&self) -> u32 {
        self.next
    }

    pub(crate) fn holds(&self, frame: u32) -> bool {
        self.intra.is_some_and(|intra| frame >= intra)
            && frame < self.next
            && self.next - frame <= self.depth
            && self.slots.iter().flatten().any(|held| held.frame == frame)
    }

    pub(crate) fn plan(&self, reference: Option<u32>, qp: Option<i32>) -> Result<Plan> {
        let frame = self.next;
        let reference = match reference {
            Some(wanted) if !self.holds(wanted) => return Err(Error::Evicted(wanted)),
            other => other,
        };
        let intra = reference.is_none() || self.intra.is_none();
        let base = match intra {
            true => frame,
            false => self.intra.unwrap_or(frame),
        };
        let wanted = reference.unwrap_or(frame);
        let kept: Vec<Reference> = match intra {
            true => Vec::new(),
            false => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(slot, held)| {
                    let held = (*held)?;
                    (self.holds(held.frame) && held.frame <= wanted).then_some(Reference {
                        slot: slot as u8,
                        order: held.frame - base,
                        intra: held.intra,
                    })
                })
                .collect(),
        };
        let reference = match intra {
            true => None,
            false => kept
                .iter()
                .copied()
                .find(|kept| kept.order + base == wanted),
        };
        let setup_slot = (0..self.slots.len())
            .find(|slot| intra || !kept.iter().any(|kept| usize::from(kept.slot) == *slot))
            .unwrap_or(0) as u8;
        Ok(Plan {
            intra,
            order: frame - base,
            idr_id: self.idr_id,
            setup_slot,
            reference,
            kept,
            qp,
        })
    }

    pub(crate) fn commit(&mut self, plan: &Plan) -> u32 {
        let frame = self.next;
        if plan.intra {
            self.slots.iter_mut().for_each(|slot| *slot = None);
            self.intra = Some(frame);
            self.idr_id = self.idr_id.wrapping_add(1);
        }
        if let Some(reference) = plan.reference {
            let reference = frame - plan.order + reference.order;
            for slot in &mut self.slots {
                if slot.is_some_and(|held| held.frame > reference) {
                    *slot = None;
                }
            }
        }
        self.slots[usize::from(plan.setup_slot)] = Some(Held {
            frame,
            intra: plan.intra,
        });
        self.next += 1;
        frame
    }
}
