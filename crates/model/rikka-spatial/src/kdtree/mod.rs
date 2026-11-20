mod duplicates;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::ControlFlow;

use glam::Vec3;
use smallvec::SmallVec;

pub use duplicates::Duplicates;

pub(crate) const UNSET: u32 = u32::MAX;

#[derive(Debug, Clone, Copy)]
pub(crate) struct KdNode {
    pub(crate) position: Vec3,
    pub(crate) index: u32,
    pub(crate) left: u32,
    pub(crate) right: u32,
    pub(crate) axis: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointNearest {
    pub index: u32,
    pub position: Vec3,
    pub distance: f32,
}

#[derive(Debug, Clone)]
pub struct PointTree {
    nodes: Vec<KdNode>,
    root: u32,
    index_bound: usize,
}

pub(crate) fn distance_squared(a: Vec3, b: Vec3) -> f32 {
    let difference = a - b;
    difference.x * difference.x + difference.y * difference.y + difference.z * difference.z
}

impl PointTree {
    pub fn new(points: impl IntoIterator<Item = Vec3>) -> Self {
        Self::from_indexed(points.into_iter().enumerate().map(|(index, point)| {
            (
                u32::try_from(index).expect("point count exceeds u32"),
                point,
            )
        }))
    }

    pub fn from_indexed(points: impl IntoIterator<Item = (u32, Vec3)>) -> Self {
        let mut nodes: Vec<KdNode> = points
            .into_iter()
            .map(|(index, position)| KdNode {
                position,
                index,
                left: UNSET,
                right: UNSET,
                axis: 0,
            })
            .collect();
        let index_bound = nodes
            .iter()
            .map(|node| node.index as usize + 1)
            .max()
            .unwrap_or(0);
        let root = balance(&mut nodes, 0, 0);
        Self {
            nodes,
            root,
            index_bound,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nearest(&self, point: Vec3) -> Option<PointNearest> {
        self.nearest_filtered(point, |_| true)
    }

    pub fn nearest_filtered(
        &self,
        point: Vec3,
        mut filter: impl FnMut(u32) -> bool,
    ) -> Option<PointNearest> {
        let mut best: Option<(f32, u32, u32)> = None;
        self.search(point, f32::INFINITY, |node_index, distance| {
            let node = &self.nodes[node_index as usize];
            let better = match best {
                None => true,
                Some((best_distance, best_index, _)) => {
                    distance < best_distance
                        || (distance == best_distance && node.index < best_index)
                }
            };
            if better && filter(node.index) {
                best = Some((distance, node.index, node_index));
            }
            best.map_or(f32::INFINITY, |(best_distance, _, _)| best_distance)
        });
        best.map(|(distance, index, node)| PointNearest {
            index,
            position: self.nodes[node as usize].position,
            distance: distance.sqrt(),
        })
    }

    pub fn k_nearest(&self, point: Vec3, count: usize) -> Vec<PointNearest> {
        if count == 0 {
            return Vec::new();
        }
        let mut heap: BinaryHeap<Ranked> = BinaryHeap::with_capacity(count + 1);
        self.search(point, f32::INFINITY, |node_index, distance| {
            let node = &self.nodes[node_index as usize];
            let ranked = Ranked {
                distance,
                index: node.index,
                node: node_index,
            };
            if heap.len() < count {
                heap.push(ranked);
            } else if heap.peek().is_some_and(|worst| ranked < *worst) {
                heap.pop();
                heap.push(ranked);
            }
            match heap.len() == count {
                true => heap.peek().map_or(f32::INFINITY, |worst| worst.distance),
                false => f32::INFINITY,
            }
        });
        heap.into_sorted_vec()
            .into_iter()
            .map(|ranked| self.result(ranked))
            .collect()
    }

    pub fn within_radius(&self, point: Vec3, radius: f32) -> Vec<PointNearest> {
        let mut found = Vec::new();
        self.for_each_within_radius(point, radius, |nearest| {
            found.push(nearest);
            ControlFlow::Continue(())
        });
        found.sort_by(|left, right| {
            left.distance
                .total_cmp(&right.distance)
                .then(left.index.cmp(&right.index))
        });
        found
    }

    pub fn for_each_within_radius(
        &self,
        point: Vec3,
        radius: f32,
        mut visit: impl FnMut(PointNearest) -> ControlFlow<()>,
    ) {
        let _ = self.range_search(point, radius, |node| {
            let node = &self.nodes[node as usize];
            visit(PointNearest {
                index: node.index,
                position: node.position,
                distance: distance_squared(node.position, point).sqrt(),
            })
        });
    }

    fn result(&self, ranked: Ranked) -> PointNearest {
        PointNearest {
            index: ranked.index,
            position: self.nodes[ranked.node as usize].position,
            distance: ranked.distance.sqrt(),
        }
    }

    fn search(&self, point: Vec3, limit: f32, mut offer: impl FnMut(u32, f32) -> f32) {
        if self.root == UNSET {
            return;
        }
        let mut bound = limit;
        let mut stack: SmallVec<[(u32, f32); 64]> = SmallVec::new();
        stack.push((self.root, 0.0));
        while let Some((node_index, lower)) = stack.pop() {
            if lower > bound {
                continue;
            }
            let node = &self.nodes[node_index as usize];
            let distance = distance_squared(node.position, point);
            if distance <= bound {
                bound = offer(node_index, distance);
            }
            let axis = node.axis as usize;
            let offset = point[axis] - node.position[axis];
            let (near, far) = match offset < 0.0 {
                true => (node.left, node.right),
                false => (node.right, node.left),
            };
            let far_bound = offset * offset;
            if far != UNSET && far_bound <= bound {
                stack.push((far, far_bound));
            }
            if near != UNSET {
                stack.push((near, 0.0));
            }
        }
    }

    pub(crate) fn range_search(
        &self,
        point: Vec3,
        radius: f32,
        mut visit: impl FnMut(u32) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if self.root == UNSET {
            return ControlFlow::Continue(());
        }
        let radius_squared = radius * radius;
        let mut stack: SmallVec<[u32; 100]> = SmallVec::new();
        stack.push(self.root);
        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index as usize];
            let axis = node.axis as usize;
            if point[axis] + radius < node.position[axis] {
                if node.left != UNSET {
                    stack.push(node.left);
                }
            } else if point[axis] - radius > node.position[axis] {
                if node.right != UNSET {
                    stack.push(node.right);
                }
            } else {
                if distance_squared(node.position, point) <= radius_squared {
                    visit(node_index)?;
                }
                if node.left != UNSET {
                    stack.push(node.left);
                }
                if node.right != UNSET {
                    stack.push(node.right);
                }
            }
        }
        ControlFlow::Continue(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Ranked {
    distance: f32,
    index: u32,
    node: u32,
}

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Ranked {}

impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then(self.index.cmp(&other.index))
    }
}

fn balance(nodes: &mut [KdNode], axis: usize, offset: u32) -> u32 {
    let count = nodes.len();
    if count == 0 {
        return UNSET;
    }
    if count == 1 {
        return offset;
    }
    let median = count / 2;
    let mut left = 0usize;
    let mut right = count - 1;
    while right > left {
        let pivot = nodes[right].position[axis];
        let mut i = left as isize - 1;
        let mut j = right as isize;
        loop {
            loop {
                i += 1;
                if nodes[i as usize].position[axis].partial_cmp(&pivot) != Some(Ordering::Less) {
                    break;
                }
            }
            loop {
                j -= 1;
                if !(nodes[j as usize].position[axis] > pivot && j > left as isize) {
                    break;
                }
            }
            if i >= j {
                break;
            }
            nodes.swap(i as usize, j as usize);
        }
        let i = i as usize;
        nodes.swap(i, right);
        if i >= median {
            right = i - 1;
        }
        if i <= median {
            left = i + 1;
        }
    }
    nodes[median].axis = axis as u8;
    let next = (axis + 1) % 3;
    let (lower, rest) = nodes.split_at_mut(median);
    let upper = &mut rest[1..];
    let left_child = balance(lower, next, offset);
    let right_child = balance(upper, next, offset + median as u32 + 1);
    nodes[median].left = left_child;
    nodes[median].right = right_child;
    offset + median as u32
}
