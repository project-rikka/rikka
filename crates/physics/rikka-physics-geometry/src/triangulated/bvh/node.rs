use bytemuck::{Pod, Zeroable};
use rikka_physics_math::Aabb;

use crate::convex::extrema::{larger, smaller};

pub const EMPTY_CHILD: u32 = u32::MAX;
pub const MAX_LEAF_SIZE: u32 = 16;
const LEAF_BIT: u32 = 1 << 31;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Child {
    #[default]
    Empty,
    Node(u32),
    Leaf {
        start: u32,
        count: u32,
    },
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Node4 {
    pub min_x: [f32; 4],
    pub min_y: [f32; 4],
    pub min_z: [f32; 4],
    pub max_x: [f32; 4],
    pub max_y: [f32; 4],
    pub max_z: [f32; 4],
    pub children: [u32; 4],
    pub counts: [u32; 4],
}

const _: () = assert!(size_of::<Node4>() == 128);

impl Default for Node4 {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Node4 {
    pub const EMPTY: Self = Self {
        min_x: [f32::MAX; 4],
        min_y: [f32::MAX; 4],
        min_z: [f32::MAX; 4],
        max_x: [-f32::MAX; 4],
        max_y: [-f32::MAX; 4],
        max_z: [-f32::MAX; 4],
        children: [EMPTY_CHILD; 4],
        counts: [0; 4],
    };

    #[inline]
    pub fn child(&self, slot: usize) -> Child {
        let child = self.children[slot];
        let count = self.counts[slot];
        if child == EMPTY_CHILD {
            Child::Empty
        } else if count == 0 {
            Child::Node(child)
        } else {
            Child::Leaf {
                start: child,
                count,
            }
        }
    }

    #[inline]
    pub fn bounds(&self, slot: usize) -> Aabb {
        Aabb::new(
            [self.min_x[slot], self.min_y[slot], self.min_z[slot]].into(),
            [self.max_x[slot], self.max_y[slot], self.max_z[slot]].into(),
        )
    }

    #[inline]
    pub fn set_bounds(&mut self, slot: usize, bounds: Aabb) {
        self.min_x[slot] = bounds.min.x;
        self.min_y[slot] = bounds.min.y;
        self.min_z[slot] = bounds.min.z;
        self.max_x[slot] = bounds.max.x;
        self.max_y[slot] = bounds.max.y;
        self.max_z[slot] = bounds.max.z;
    }

    #[inline]
    pub fn set_child(&mut self, slot: usize, child: Child) {
        let (reference, count) = match child {
            Child::Empty => (EMPTY_CHILD, 0),
            Child::Node(index) => (index, 0),
            Child::Leaf { start, count } => (start, count),
        };
        self.children[slot] = reference;
        self.counts[slot] = count;
    }

    pub fn union(&self) -> Aabb {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut min_z = f32::MAX;
        let mut max_x = -f32::MAX;
        let mut max_y = -f32::MAX;
        let mut max_z = -f32::MAX;
        for slot in 0..4 {
            if self.children[slot] != EMPTY_CHILD {
                min_x = smaller(min_x, self.min_x[slot]);
                min_y = smaller(min_y, self.min_y[slot]);
                min_z = smaller(min_z, self.min_z[slot]);
                max_x = larger(max_x, self.max_x[slot]);
                max_y = larger(max_y, self.max_y[slot]);
                max_z = larger(max_z, self.max_z[slot]);
            }
        }
        Aabb::new([min_x, min_y, min_z].into(), [max_x, max_y, max_z].into())
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct QuantizedNode4 {
    pub lower: [[u16; 4]; 3],
    pub inset: [[u16; 4]; 3],
    pub children: [u32; 4],
}

const _: () = assert!(size_of::<QuantizedNode4>() == 64);

impl Default for QuantizedNode4 {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl QuantizedNode4 {
    pub const EMPTY: Self = Self {
        lower: [[u16::MAX; 4]; 3],
        inset: [[u16::MAX; 4]; 3],
        children: [EMPTY_CHILD; 4],
    };

    #[inline]
    pub fn child(&self, slot: usize) -> Child {
        decode_child(self.children[slot])
    }
}

#[inline]
pub(crate) fn decode_child(reference: u32) -> Child {
    if reference == EMPTY_CHILD {
        Child::Empty
    } else if reference & LEAF_BIT == 0 {
        Child::Node(reference)
    } else {
        Child::Leaf {
            start: (reference & !LEAF_BIT) >> 4,
            count: (reference & 15) + 1,
        }
    }
}

#[inline]
pub(crate) fn encode_child(child: Child) -> u32 {
    match child {
        Child::Empty => EMPTY_CHILD,
        Child::Node(index) => {
            debug_assert!(index < LEAF_BIT);
            index
        }
        Child::Leaf { start, count } => {
            debug_assert!((1..=MAX_LEAF_SIZE).contains(&count) && start < 1 << 27);
            LEAF_BIT | start << 4 | (count - 1)
        }
    }
}
