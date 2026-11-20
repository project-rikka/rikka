use crate::graph::Edge;

pub(crate) const EVENTS: u32 = 1 << 0;
pub(crate) const HIT_EVENTS: u32 = 1 << 1;
pub(crate) const SKIP: u32 = 1 << 2;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ContactRecord {
    pub shape_a: u32,
    pub shape_b: u32,
    pub edges: [Edge; 2],
    pub set: u32,
    pub set_index: u32,
    pub color: u32,
    pub color_index: u32,
    pub island: u32,
    pub island_prev: u32,
    pub island_next: u32,
    pub solver_a: u32,
    pub solver_b: u32,
    pub flags: u32,
}

impl ContactRecord {
    #[inline]
    pub(crate) fn body_a(&self) -> u32 {
        self.edges[0].body
    }

    #[inline]
    pub(crate) fn body_b(&self) -> u32 {
        self.edges[1].body
    }
}
