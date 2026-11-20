use rikka_physics_core::BitSet;

pub(crate) const COLOR_COUNT: usize = 24;
pub(crate) const OVERFLOW: u32 = COLOR_COUNT as u32 - 1;
const DYNAMIC_COLORS: u32 = COLOR_COUNT as u32 - 4;
const BALANCE_SLACK: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Edge {
    pub body: u32,
    pub prev: u32,
    pub next: u32,
}

#[derive(Debug, Default)]
pub(crate) struct GraphColor {
    pub bodies: BitSet,
    pub contacts: Vec<u32>,
    pub joints: Vec<u32>,
}

impl GraphColor {
    fn size(&self) -> usize {
        self.contacts.len() + self.joints.len()
    }

    fn is_free(&self, body: u32) -> bool {
        (body as usize) >= self.bodies.capacity() || !self.bodies.get(body as usize)
    }

    fn claim(&mut self, body: u32) {
        self.bodies.set_growing(body as usize);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Constraint {
    Contact,
    Joint,
}

#[derive(Debug)]
pub(crate) struct ConstraintGraph {
    pub colors: Vec<GraphColor>,
    pub version: u64,
}

impl Default for ConstraintGraph {
    fn default() -> Self {
        Self {
            colors: (0..COLOR_COUNT).map(|_| GraphColor::default()).collect(),
            version: 0,
        }
    }
}

impl ConstraintGraph {
    fn assign(&mut self, a: (u32, bool), b: (u32, bool)) -> u32 {
        match (a.1, b.1) {
            (true, true) => {
                let mut first = None;
                for color in 0..DYNAMIC_COLORS {
                    let slot = &self.colors[color as usize];
                    if !slot.is_free(a.0) || !slot.is_free(b.0) {
                        continue;
                    }
                    match first {
                        None => first = Some(color),
                        Some(chosen) => {
                            let size = self.colors[chosen as usize].size();
                            let pick = if size > 2 * slot.size() + BALANCE_SLACK {
                                color
                            } else {
                                chosen
                            };
                            first = Some(pick);
                            break;
                        }
                    }
                }
                match first {
                    Some(color) => {
                        self.colors[color as usize].claim(a.0);
                        self.colors[color as usize].claim(b.0);
                        color
                    }
                    None => OVERFLOW,
                }
            }
            (true, false) | (false, true) => {
                let body = if a.1 { a.0 } else { b.0 };
                for color in (1..OVERFLOW).rev() {
                    if self.colors[color as usize].is_free(body) {
                        self.colors[color as usize].claim(body);
                        return color;
                    }
                }
                OVERFLOW
            }
            (false, false) => OVERFLOW,
        }
    }

    pub(crate) fn add(
        &mut self,
        constraint: Constraint,
        id: u32,
        a: (u32, bool),
        b: (u32, bool),
    ) -> (u32, u32) {
        self.version += 1;
        let color = self.assign(a, b);
        let list = self.list(constraint, color);
        list.push(id);
        (color, list.len() as u32 - 1)
    }

    pub(crate) fn remove(
        &mut self,
        constraint: Constraint,
        color: u32,
        index: u32,
        a: u32,
        b: u32,
    ) -> Option<u32> {
        self.version += 1;
        let slot = &mut self.colors[color as usize];
        if color != OVERFLOW {
            if (a as usize) < slot.bodies.capacity() {
                slot.bodies.clear(a as usize);
            }
            if (b as usize) < slot.bodies.capacity() {
                slot.bodies.clear(b as usize);
            }
        }
        let list = self.list(constraint, color);
        list.swap_remove(index as usize);
        list.get(index as usize).copied()
    }

    fn list(&mut self, constraint: Constraint, color: u32) -> &mut Vec<u32> {
        let slot = &mut self.colors[color as usize];
        match constraint {
            Constraint::Contact => &mut slot.contacts,
            Constraint::Joint => &mut slot.joints,
        }
    }
}
