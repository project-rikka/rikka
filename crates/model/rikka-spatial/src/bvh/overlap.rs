use glam::Affine3A;
use smallvec::SmallVec;

use super::{Bvh, Node};
use crate::aabb::Aabb;

struct Transfer {
    forward: Affine3A,
    backward: Affine3A,
}

impl Transfer {
    fn overlaps(&self, bounds: &Aabb, other: &Aabb) -> bool {
        bounds.intersects(&other.transformed(&self.forward))
            && other.intersects(&bounds.transformed(&self.backward))
    }
}

impl Bvh {
    pub fn for_each_overlap(
        &self,
        other: &Bvh,
        transform: &Affine3A,
        mut visit: impl FnMut(u32, u32),
    ) {
        if self.nodes.is_empty() || other.nodes.is_empty() {
            return;
        }
        let transfer = Transfer {
            forward: *transform,
            backward: transform.inverse(),
        };
        let mut stack: SmallVec<[(u32, u32); 64]> = SmallVec::new();
        stack.push((0, 0));
        while let Some((index, other_index)) = stack.pop() {
            let node = &self.nodes[index as usize];
            let other_node = &other.nodes[other_index as usize];
            if !transfer.overlaps(&node.bounds(), &other_node.bounds()) {
                continue;
            }
            match (node.is_leaf(), other_node.is_leaf()) {
                (true, true) => {
                    for primitive in self.leaf(node) {
                        for other_primitive in other.leaf(other_node) {
                            if transfer.overlaps(&primitive.bounds(), &other_primitive.bounds()) {
                                visit(primitive.id, other_primitive.id);
                            }
                        }
                    }
                }
                (false, true) => descend_first(&mut stack, node, other_index),
                (true, false) => descend_second(&mut stack, index, other_node),
                (false, false) => {
                    let area = node.bounds().surface_area();
                    let other_area = other_node
                        .bounds()
                        .transformed(&transfer.forward)
                        .surface_area();
                    match area >= other_area {
                        true => descend_first(&mut stack, node, other_index),
                        false => descend_second(&mut stack, index, other_node),
                    }
                }
            }
        }
    }

    pub fn for_each_self_overlap(&self, mut visit: impl FnMut(u32, u32)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut report = |first: u32, second: u32| visit(first.min(second), first.max(second));
        let mut stack: SmallVec<[(u32, u32); 64]> = SmallVec::new();
        stack.push((0, 0));
        while let Some((index, other_index)) = stack.pop() {
            let node = &self.nodes[index as usize];
            if index == other_index {
                if node.is_leaf() {
                    let primitives = self.leaf(node);
                    for (position, primitive) in primitives.iter().enumerate() {
                        for other_primitive in &primitives[position + 1..] {
                            if primitive.bounds().intersects(&other_primitive.bounds()) {
                                report(primitive.id, other_primitive.id);
                            }
                        }
                    }
                } else {
                    stack.push((node.start, node.start + 1));
                    stack.push((node.start + 1, node.start + 1));
                    stack.push((node.start, node.start));
                }
                continue;
            }
            let other_node = &self.nodes[other_index as usize];
            if !node.bounds().intersects(&other_node.bounds()) {
                continue;
            }
            match (node.is_leaf(), other_node.is_leaf()) {
                (true, true) => {
                    for primitive in self.leaf(node) {
                        for other_primitive in self.leaf(other_node) {
                            if primitive.bounds().intersects(&other_primitive.bounds()) {
                                report(primitive.id, other_primitive.id);
                            }
                        }
                    }
                }
                (false, true) => descend_first(&mut stack, node, other_index),
                (true, false) => descend_second(&mut stack, index, other_node),
                (false, false) => {
                    match node.bounds().surface_area() >= other_node.bounds().surface_area() {
                        true => descend_first(&mut stack, node, other_index),
                        false => descend_second(&mut stack, index, other_node),
                    }
                }
            }
        }
    }
}

fn descend_first(stack: &mut SmallVec<[(u32, u32); 64]>, node: &Node, other_index: u32) {
    stack.push((node.start + 1, other_index));
    stack.push((node.start, other_index));
}

fn descend_second(stack: &mut SmallVec<[(u32, u32); 64]>, index: u32, other_node: &Node) {
    stack.push((index, other_node.start + 1));
    stack.push((index, other_node.start));
}
