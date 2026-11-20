use rikka_physics_math::{Aabb, Vec3};

use super::node::{Child, Node4};
use super::stack::Stack;

const MAX_BINS: usize = 32;

pub(crate) trait NodeAlloc {
    fn allocate(&mut self) -> u32;
    fn node_mut(&mut self, index: u32) -> &mut Node4;
}

impl NodeAlloc for Vec<Node4> {
    #[inline]
    fn allocate(&mut self) -> u32 {
        self.push(Node4::EMPTY);
        (self.len() - 1) as u32
    }

    #[inline]
    fn node_mut(&mut self, index: u32) -> &mut Node4 {
        &mut self[index as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Task {
    pub(crate) node: u32,
    pub(crate) start: u32,
    pub(crate) count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Work {
    task: Task,
    bounds: Aabb,
    centroids: Aabb,
}

#[derive(Clone, Copy, Debug)]
struct Cluster {
    start: u32,
    count: u32,
    bounds: Aabb,
    centroids: Aabb,
}

#[derive(Clone, Copy, Debug)]
struct Split {
    index: usize,
    left_bounds: Aabb,
    left_centroids: Aabb,
    right_bounds: Aabb,
    right_centroids: Aabb,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Builder<'a> {
    bounds: &'a [Aabb],
    leaf_size: u32,
    bins: usize,
}

impl<'a> Builder<'a> {
    pub(crate) fn new(bounds: &'a [Aabb], leaf_size: u32, bins: u32) -> Self {
        Self {
            bounds,
            leaf_size: leaf_size.max(1),
            bins: (bins as usize).clamp(2, MAX_BINS),
        }
    }

    #[inline]
    fn item_bounds(&self, item: u32) -> Aabb {
        self.bounds[item as usize]
    }

    fn bounds_and_centroids(&self, items: &[u32]) -> (Aabb, Aabb) {
        let mut bounds = Aabb::EMPTY;
        let mut centroids = Aabb::EMPTY;
        for &item in items {
            let item_bounds = self.item_bounds(item);
            bounds = bounds.union(&item_bounds);
            centroids = centroids.including(item_bounds.center());
        }
        (bounds, centroids)
    }

    pub(crate) fn build(
        &self,
        items: &mut [u32],
        base: u32,
        root: u32,
        nodes: &mut impl NodeAlloc,
        grain: Option<u32>,
        deferred: &mut Vec<Task>,
    ) {
        let (bounds, centroids) = self.bounds_and_centroids(items);
        let mut work = Stack::new();
        work.push(Work {
            task: Task {
                node: root,
                start: base,
                count: items.len() as u32,
            },
            bounds,
            centroids,
        });
        while let Some(entry) = work.pop() {
            let task = entry.task;
            if let Some(grain) = grain
                && task.count <= grain
                && task.node != root
            {
                deferred.push(task);
                continue;
            }
            let offset = (task.start - base) as usize;
            let range = &mut items[offset..offset + task.count as usize];
            let clusters = self.partition4(range, task.start, entry.bounds, entry.centroids);
            let mut node = Node4::EMPTY;
            let mut children = [None; 4];
            for (slot, cluster) in clusters.iter().enumerate() {
                let Some(cluster) = cluster else {
                    continue;
                };
                node.set_bounds(slot, cluster.bounds);
                if cluster.count <= self.leaf_size {
                    node.set_child(
                        slot,
                        Child::Leaf {
                            start: cluster.start,
                            count: cluster.count,
                        },
                    );
                } else {
                    let child = nodes.allocate();
                    node.set_child(slot, Child::Node(child));
                    children[slot] = Some(Work {
                        task: Task {
                            node: child,
                            start: cluster.start,
                            count: cluster.count,
                        },
                        bounds: cluster.bounds,
                        centroids: cluster.centroids,
                    });
                }
            }
            *nodes.node_mut(task.node) = node;
            for child in children.into_iter().rev().flatten() {
                work.push(child);
            }
        }
    }

    fn partition4(
        &self,
        items: &mut [u32],
        start: u32,
        bounds: Aabb,
        centroids: Aabb,
    ) -> [Option<Cluster>; 4] {
        let mut clusters = [None; 4];
        clusters[0] = Some(Cluster {
            start,
            count: items.len() as u32,
            bounds,
            centroids,
        });
        let mut used = 1;
        while used < 4 {
            let mut chosen = None;
            let mut best_area = -1.0;
            for (index, cluster) in clusters[..used].iter().enumerate() {
                let Some(cluster) = cluster else {
                    continue;
                };
                let area = cluster.bounds.surface_area();
                if cluster.count > self.leaf_size && (chosen.is_none() || area > best_area) {
                    best_area = area;
                    chosen = Some(index);
                }
            }
            let Some(index) = chosen else {
                break;
            };
            let Some(cluster) = clusters[index] else {
                break;
            };
            let local = (cluster.start - start) as usize;
            let range = &mut items[local..local + cluster.count as usize];
            let split = self.split(range, cluster.centroids);
            for moved in (index + 1..used).rev() {
                clusters[moved + 1] = clusters[moved];
            }
            clusters[index] = Some(Cluster {
                start: cluster.start,
                count: split.index as u32,
                bounds: split.left_bounds,
                centroids: split.left_centroids,
            });
            clusters[index + 1] = Some(Cluster {
                start: cluster.start + split.index as u32,
                count: (cluster.count as usize - split.index) as u32,
                bounds: split.right_bounds,
                centroids: split.right_centroids,
            });
            used += 1;
        }
        clusters
    }

    fn split_half(&self, items: &[u32]) -> Split {
        let index = items.len() / 2;
        let (left_bounds, left_centroids) = self.bounds_and_centroids(&items[..index]);
        let (right_bounds, right_centroids) = self.bounds_and_centroids(&items[index..]);
        Split {
            index,
            left_bounds,
            left_centroids,
            right_bounds,
            right_centroids,
        }
    }

    fn split(&self, items: &mut [u32], centroids: Aabb) -> Split {
        let count = items.len();
        let extent = centroids.size();
        let mut active = [false; 3];
        let mut factors = [0.0f32; 3];
        let scale = self.bins as f32 * (1.0 - f32::EPSILON);
        for axis in 0..3 {
            if extent[axis] > f32::EPSILON * centroids.max[axis].abs().max(1.0) {
                active[axis] = true;
                factors[axis] = scale / extent[axis];
            }
        }
        if !active[0] && !active[1] && !active[2] {
            return self.split_half(items);
        }

        let mut bin_counts = [[0u32; MAX_BINS]; 3];
        let mut bin_bounds = [[Aabb::EMPTY; MAX_BINS]; 3];
        for &item in items.iter() {
            let bounds = self.item_bounds(item);
            let center = bounds.center();
            for axis in 0..3 {
                if active[axis] {
                    let bin = bin_index(center, &centroids, axis, factors[axis], self.bins);
                    bin_counts[axis][bin] += 1;
                    bin_bounds[axis][bin] = bin_bounds[axis][bin].union(&bounds);
                }
            }
        }

        let mut best_cost = f32::MAX;
        let mut best: Option<(usize, usize)> = None;
        for axis in 0..3 {
            if !active[axis] {
                continue;
            }
            let counts = &bin_counts[axis];
            let bounds = &bin_bounds[axis];
            let mut right_area = [0.0f32; MAX_BINS];
            let mut right_count = [0u32; MAX_BINS];
            let mut accumulated = Aabb::EMPTY;
            let mut total = 0;
            for bin in (1..self.bins).rev() {
                accumulated = accumulated.union(&bounds[bin]);
                total += counts[bin];
                right_area[bin] = if total > 0 {
                    accumulated.surface_area()
                } else {
                    0.0
                };
                right_count[bin] = total;
            }
            let mut left = Aabb::EMPTY;
            let mut left_count = 0;
            for bin in 0..self.bins - 1 {
                left = left.union(&bounds[bin]);
                left_count += counts[bin];
                let right = right_count[bin + 1];
                if left_count == 0 || right == 0 {
                    continue;
                }
                let cost =
                    left_count as f32 * left.surface_area() + right as f32 * right_area[bin + 1];
                if cost < best_cost {
                    best_cost = cost;
                    best = Some((axis, bin));
                }
            }
        }

        let Some((axis, best_bin)) = best else {
            return self.split_half(items);
        };
        let factor = factors[axis];
        let mut split = 0;
        let mut left_bounds = Aabb::EMPTY;
        let mut left_centroids = Aabb::EMPTY;
        let mut right_bounds = Aabb::EMPTY;
        let mut right_centroids = Aabb::EMPTY;
        for index in 0..count {
            let bounds = self.item_bounds(items[index]);
            let center = bounds.center();
            if bin_index(center, &centroids, axis, factor, self.bins) <= best_bin {
                items.swap(index, split);
                split += 1;
                left_bounds = left_bounds.union(&bounds);
                left_centroids = left_centroids.including(center);
            } else {
                right_bounds = right_bounds.union(&bounds);
                right_centroids = right_centroids.including(center);
            }
        }
        if split == 0 || split == count {
            self.split_half(items)
        } else {
            Split {
                index: split,
                left_bounds,
                left_centroids,
                right_bounds,
                right_centroids,
            }
        }
    }
}

#[inline]
fn bin_index(center: Vec3, centroids: &Aabb, axis: usize, factor: f32, bins: usize) -> usize {
    let offset = (center[axis] - centroids.min[axis]) * factor;
    (offset.max(0.0) as usize).min(bins - 1)
}
