use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{BodyId, RigidWorld};

const ABSENT: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    body: BodyId,
    from: Transform,
    to: Transform,
    last: Transform,
    reached: bool,
    settle: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Targets {
    list: Vec<Target>,
    slots: Vec<u32>,
}

impl Targets {
    pub(crate) fn set(&mut self, body: BodyId, current: Transform, to: Transform) {
        match self.position(body) {
            Some(at) => {
                let target = &mut self.list[at];
                target.from = target.last;
                target.to = to;
                target.reached = false;
                target.settle = false;
            }
            None => {
                let index = body.index as usize;
                if self.slots.len() <= index {
                    self.slots.resize(index + 1, ABSENT);
                }
                self.slots[index] = self.list.len() as u32;
                self.list.push(Target {
                    body,
                    from: current,
                    to,
                    last: current,
                    reached: false,
                    settle: false,
                });
            }
        }
    }

    pub(crate) fn remove(&mut self, body: BodyId) {
        let Some(at) = self.position(body) else {
            return;
        };
        self.slots[body.index as usize] = ABSENT;
        self.list.remove(at);
        self.renumber(at);
    }

    pub(crate) fn clear(&mut self) {
        self.list.clear();
        self.slots.clear();
    }

    pub(crate) fn apply(&mut self, rigid: &mut RigidWorld, fraction: f32) {
        let mut settled = false;
        for target in &mut self.list {
            if target.reached {
                target.settle = true;
                settled = true;
                target.last = target.to;
            } else {
                target.last = target.from.lerp(&target.to, fraction);
                target.reached = fraction >= 1.0;
            }
            rigid
                .move_to(target.body, target.last)
                .expect("target body is a live kinematic body");
        }
        if settled {
            for target in self.list.iter().filter(|target| target.settle) {
                self.slots[target.body.index as usize] = ABSENT;
            }
            self.list.retain(|target| !target.settle);
            self.renumber(0);
        }
    }

    pub(crate) fn finish_frame(&mut self) {
        for target in &mut self.list {
            target.from = target.last;
        }
    }

    pub(crate) fn snap(&mut self, rigid: &mut RigidWorld) {
        for target in &self.list {
            rigid
                .set_pose(target.body, target.to)
                .expect("target body is a live kinematic body");
        }
        self.clear();
    }

    pub(crate) fn shift(&mut self, offset: Vec3) {
        for target in &mut self.list {
            target.from.p -= offset;
            target.to.p -= offset;
            target.last.p -= offset;
        }
    }

    fn position(&self, body: BodyId) -> Option<usize> {
        let at = *self.slots.get(body.index as usize)? as usize;
        self.list
            .get(at)
            .filter(|target| target.body == body)
            .map(|_| at)
    }

    fn renumber(&mut self, from: usize) {
        for (at, target) in self.list.iter().enumerate().skip(from) {
            self.slots[target.body.index as usize] = at as u32;
        }
    }
}
