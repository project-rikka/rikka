use rikka_physics_math::{Aabb, F32x4, Float, MaskW, Vec3, Vector};

use super::bounds4::{Bounds4, Point4, QueryBounds4, Sweep4};
use super::node::Child;
use super::stack::Stack;
use crate::convex::extrema::smaller;

const QUERY_STACK: usize = 48;

pub(crate) trait Nodes {
    type Frame: Copy + Default;

    fn root(&self) -> Option<(u32, Self::Frame)>;
    fn load(&self, node: u32, frame: &Self::Frame) -> Bounds4;
    fn child(&self, node: u32, slot: usize) -> Child;
    fn occupied(&self, node: u32) -> u32;
    fn child_frame(bounds: &Bounds4, slot: usize) -> Self::Frame;
}

#[derive(Clone, Copy, Debug, Default)]
struct Entry<F> {
    child: Child,
    frame: F,
    key: f32,
}

#[inline]
pub(crate) fn slots(mask: u32) -> impl DoubleEndedIterator<Item = usize> {
    (0..4).filter(move |slot| mask & (1 << slot) != 0)
}

pub(crate) fn query<N: Nodes>(
    nodes: &N,
    bounds: &Aabb,
    mut visit: impl FnMut(u32) -> bool,
) -> bool {
    query_leaves(nodes, bounds, |start, count| {
        (start..start + count).all(&mut visit)
    })
}

pub(crate) fn query_leaves<N: Nodes>(
    nodes: &N,
    bounds: &Aabb,
    visit: impl FnMut(u32, u32) -> bool,
) -> bool {
    query_leaves_splatted(nodes, &QueryBounds4::new(bounds), visit)
}

pub(crate) fn query_leaves_splatted<N: Nodes>(
    nodes: &N,
    query: &QueryBounds4,
    mut visit: impl FnMut(u32, u32) -> bool,
) -> bool {
    let Some((root, frame)) = nodes.root() else {
        return true;
    };
    let mut stack = Stack::<(u32, N::Frame), QUERY_STACK>::empty();
    stack.push((root, frame));
    while let Some((node, frame)) = stack.pop() {
        let children = nodes.load(node, &frame);
        let mut mask = children.overlaps_query(query) & nodes.occupied(node);
        while mask != 0 {
            let slot = (31 - mask.leading_zeros()) as usize;
            mask ^= 1 << slot;
            match nodes.child(node, slot) {
                Child::Empty => {}
                Child::Node(child) => stack.push((child, N::child_frame(&children, slot))),
                Child::Leaf { start, count } => {
                    if !visit(start, count) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

pub(crate) fn sweep<N: Nodes>(
    nodes: &N,
    origin: Vec3,
    translation: Vec3,
    extent: Vec3,
    max_fraction: f32,
    mut visit: impl FnMut(u32, f32) -> f32,
) -> f32 {
    let mut best = max_fraction;
    let Some((root, frame)) = nodes.root() else {
        return best;
    };
    let ray = Sweep4::new(origin, translation, extent);
    let mut stack = Stack::<Entry<N::Frame>>::new();
    stack.push(Entry {
        child: Child::Node(root),
        frame,
        key: 0.0,
    });
    while let Some(entry) = stack.pop() {
        if entry.key > best {
            continue;
        }
        match entry.child {
            Child::Empty => {}
            Child::Leaf { start, count } => {
                for item in start..start + count {
                    best = smaller(visit(item, best), best);
                }
            }
            Child::Node(node) => {
                let children = nodes.load(node, &entry.frame);
                let (mask, lower) = children.sweep(&ray, best);
                let mask = mask & nodes.occupied(node);
                push_ordered(nodes, node, &children, mask, lower, &mut stack);
            }
        }
    }
    best
}

pub(crate) fn closest<N: Nodes>(
    nodes: &N,
    point: Vec3,
    max_distance_squared: f32,
    mut visit: impl FnMut(u32, f32) -> f32,
) -> f32 {
    let mut best = max_distance_squared;
    let Some((root, frame)) = nodes.root() else {
        return best;
    };
    let point = Point4::new(point);
    let mut stack = Stack::<Entry<N::Frame>>::new();
    stack.push(Entry {
        child: Child::Node(root),
        frame,
        key: 0.0,
    });
    while let Some(entry) = stack.pop() {
        if entry.key > best {
            continue;
        }
        match entry.child {
            Child::Empty => {}
            Child::Leaf { start, count } => {
                for item in start..start + count {
                    best = smaller(visit(item, best), best);
                }
            }
            Child::Node(node) => {
                let children = nodes.load(node, &entry.frame);
                let distance = children.distance_squared(&point);
                let mask = distance.simd_le(F32x4::splat(best)).bits() & nodes.occupied(node);
                push_ordered(nodes, node, &children, mask, distance, &mut stack);
            }
        }
    }
    best
}

#[inline]
fn push_ordered<N: Nodes>(
    nodes: &N,
    node: u32,
    children: &Bounds4,
    mask: u32,
    keys: F32x4,
    stack: &mut Stack<Entry<N::Frame>>,
) {
    let keys = keys.to_array::<4>();
    let mut order = [0usize; 4];
    let mut count = 0;
    for slot in slots(mask) {
        let mut position = count;
        while position > 0 && keys[order[position - 1]] < keys[slot] {
            order[position] = order[position - 1];
            position -= 1;
        }
        order[position] = slot;
        count += 1;
    }
    for &slot in &order[..count] {
        stack.push(Entry {
            child: nodes.child(node, slot),
            frame: N::child_frame(children, slot),
            key: keys[slot],
        });
    }
}
