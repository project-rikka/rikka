use rikka_physics_math::{Aabb, F32x4, Vec3, Vector};

use super::bounds4::Bounds4;
use super::node::{Child, EMPTY_CHILD, QuantizedNode4, decode_child, encode_child};
use super::traverse::{self, Nodes};

const INVERSE_RANGE: f32 = 1.0 / 65535.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuantizedBvh4 {
    pub nodes: Vec<QuantizedNode4>,
    pub bounds: Aabb,
}

impl QuantizedBvh4 {
    #[inline]
    pub fn tree(&self) -> QuantizedTree<'_> {
        QuantizedTree::new(&self.nodes, self.bounds)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct QuantizedTree<'a> {
    nodes: &'a [QuantizedNode4],
    bounds: Aabb,
}

impl<'a> QuantizedTree<'a> {
    #[inline]
    pub fn new(nodes: &'a [QuantizedNode4], bounds: Aabb) -> Self {
        Self { nodes, bounds }
    }

    #[inline]
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    #[inline]
    pub fn nodes(&self) -> &'a [QuantizedNode4] {
        self.nodes
    }

    pub fn query(&self, bounds: Aabb, visit: impl FnMut(u32) -> bool) -> bool {
        traverse::query(self, &bounds, visit)
    }

    pub fn ray_cast(
        &self,
        origin: Vec3,
        translation: Vec3,
        max_fraction: f32,
        visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        traverse::sweep(self, origin, translation, Vec3::ZERO, max_fraction, visit)
    }

    pub fn box_cast(
        &self,
        bounds: Aabb,
        translation: Vec3,
        max_fraction: f32,
        visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        traverse::sweep(
            self,
            bounds.center(),
            translation,
            bounds.extent(),
            max_fraction,
            visit,
        )
    }

    pub fn closest(
        &self,
        point: Vec3,
        max_distance: f32,
        visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        traverse::closest(self, point, max_distance * max_distance, visit)
            .max(0.0)
            .sqrt()
    }

    pub fn height(&self) -> u32 {
        let mut height = 0;
        let mut stack = vec![(0u32, 1u32)];
        if self.nodes.is_empty() {
            return 0;
        }
        while let Some((node, depth)) = stack.pop() {
            height = height.max(depth);
            for slot in 0..4 {
                if let Child::Node(child) = self.nodes[node as usize].child(slot) {
                    stack.push((child, depth + 1));
                }
            }
        }
        height
    }

    pub(crate) fn is_consistent(&self, item_count: u32) -> bool {
        self.nodes.iter().enumerate().all(|(index, node)| {
            node.children
                .iter()
                .all(|&reference| match decode_child(reference) {
                    Child::Empty => true,
                    Child::Node(child) => {
                        (child as usize) > index && (child as usize) < self.nodes.len()
                    }
                    Child::Leaf { start, count } => start
                        .checked_add(count)
                        .is_some_and(|end| end <= item_count),
                })
        })
    }
}

#[inline]
fn step(frame: &Aabb) -> Vec3 {
    (frame.max - frame.min) * INVERSE_RANGE
}

#[inline]
fn lanes(values: [u16; 4]) -> F32x4 {
    F32x4::from_array([
        f32::from(values[0]),
        f32::from(values[1]),
        f32::from(values[2]),
        f32::from(values[3]),
    ])
}

pub(crate) fn decode(node: &QuantizedNode4, frame: &Aabb) -> Bounds4 {
    let step = step(frame);
    let mut bounds = Bounds4 {
        min: [F32x4::default(); 3],
        max: [F32x4::default(); 3],
    };
    for axis in 0..3 {
        let s = F32x4::splat(step[axis]);
        bounds.min[axis] = F32x4::splat(frame.min[axis]) + lanes(node.lower[axis]) * s;
        bounds.max[axis] = F32x4::splat(frame.max[axis]) - lanes(node.inset[axis]) * s;
    }
    bounds
}

pub(crate) fn encode(frame: &Aabb, child: &Aabb) -> ([u16; 3], [u16; 3]) {
    let step = step(frame);
    let mut lower = [0u16; 3];
    let mut inset = [0u16; 3];
    for axis in 0..3 {
        let s = step[axis];
        if s <= 0.0 {
            continue;
        }
        let low = frame.min[axis];
        let high = frame.max[axis];
        let mut q = ((child.min[axis] - low) / s).floor().clamp(0.0, 65535.0) as u16;
        while q > 0 && low + f32::from(q) * s > child.min[axis] {
            q -= 1;
        }
        let mut r = ((high - child.max[axis]) / s).floor().clamp(0.0, 65535.0) as u16;
        while r > 0 && high - f32::from(r) * s < child.max[axis] {
            r -= 1;
        }
        lower[axis] = q;
        inset[axis] = r;
    }
    (lower, inset)
}

pub(crate) fn quantize_node(
    frame: &Aabb,
    children: [(Child, Aabb); 4],
) -> (QuantizedNode4, Bounds4) {
    let mut node = QuantizedNode4::EMPTY;
    for (slot, (child, bounds)) in children.into_iter().enumerate() {
        node.children[slot] = encode_child(child);
        if child == Child::Empty {
            continue;
        }
        let (lower, inset) = encode(frame, &bounds);
        for axis in 0..3 {
            node.lower[axis][slot] = lower[axis];
            node.inset[axis][slot] = inset[axis];
        }
    }
    let decoded = decode(&node, frame);
    (node, decoded)
}

impl Nodes for QuantizedTree<'_> {
    type Frame = Aabb;

    #[inline]
    fn root(&self) -> Option<(u32, Aabb)> {
        (!self.nodes.is_empty()).then_some((0, self.bounds))
    }

    #[inline]
    fn load(&self, node: u32, frame: &Aabb) -> Bounds4 {
        decode(&self.nodes[node as usize], frame)
    }

    #[inline]
    fn child(&self, node: u32, slot: usize) -> Child {
        self.nodes[node as usize].child(slot)
    }

    #[inline]
    fn occupied(&self, node: u32) -> u32 {
        let children = &self.nodes[node as usize].children;
        (0..4).fold(0, |mask, slot| {
            mask | (u32::from(children[slot] != EMPTY_CHILD) << slot)
        })
    }

    #[inline]
    fn child_frame(bounds: &Bounds4, slot: usize) -> Aabb {
        bounds.lane(slot)
    }
}
