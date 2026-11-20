use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{BodyId, BodyMoveEvent};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot {
    generation: u32,
    previous: Transform,
    current: Transform,
    stamp: i64,
}

impl Slot {
    const EMPTY: Self = Self {
        generation: u32::MAX,
        previous: Transform::IDENTITY,
        current: Transform::IDENTITY,
        stamp: i64::MIN,
    };
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BodyPoses {
    slots: Vec<Slot>,
}

impl BodyPoses {
    pub(crate) fn record(&mut self, moves: &[BodyMoveEvent], tick: i64) {
        for event in moves {
            let slot = self.slot_mut(event.body);
            slot.previous = if slot.generation == event.body.generation {
                slot.current
            } else {
                event.transform
            };
            slot.generation = event.body.generation;
            slot.current = event.transform;
            slot.stamp = tick;
        }
    }

    pub(crate) fn teleport(&mut self, body: BodyId, pose: Transform) {
        let slot = self.slot_mut(body);
        *slot = Slot {
            generation: body.generation,
            previous: pose,
            current: pose,
            stamp: i64::MIN,
        };
    }

    pub(crate) fn forget(&mut self, body: BodyId) {
        if let Some(slot) = self.slots.get_mut(body.index as usize) {
            *slot = Slot::EMPTY;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
    }

    pub(crate) fn shift(&mut self, offset: Vec3) {
        for slot in &mut self.slots {
            slot.previous.p -= offset;
            slot.current.p -= offset;
        }
    }

    pub(crate) fn interpolated(&self, body: BodyId, tick: i64, alpha: f32) -> Option<Transform> {
        let slot = self.slots.get(body.index as usize)?;
        (slot.generation == body.generation && slot.stamp == tick)
            .then(|| slot.previous.lerp(&slot.current, alpha))
    }

    fn slot_mut(&mut self, body: BodyId) -> &mut Slot {
        let index = body.index as usize;
        if index >= self.slots.len() {
            self.slots.resize(index + 1, Slot::EMPTY);
        }
        &mut self.slots[index]
    }
}
