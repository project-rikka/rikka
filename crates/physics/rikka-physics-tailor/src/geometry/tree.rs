use glam::Vec3;

use super::{Closest, closest_on_triangle};

const LEAF: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub(crate) triangle: u32,
    pub(crate) distance: f32,
    pub(crate) closest: Closest,
}

#[derive(Clone, Copy, Debug)]
struct Branch {
    low: Vec3,
    high: Vec3,
    first: u32,
    count: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct TriangleTree {
    branches: Vec<Branch>,
    order: Vec<u32>,
    corners: Vec<[Vec3; 3]>,
}

impl TriangleTree {
    pub(crate) fn new(corners: Vec<[Vec3; 3]>) -> TriangleTree {
        let mut tree = TriangleTree {
            branches: Vec::with_capacity(corners.len() / 2 + 1),
            order: (0..corners.len() as u32).collect(),
            corners,
        };
        if tree.corners.is_empty() {
            return tree;
        }
        let centres: Vec<Vec3> = tree
            .corners
            .iter()
            .map(|[a, b, c]| (*a + *b + *c) / 3.0)
            .collect();
        tree.branches.push(Branch {
            low: Vec3::ZERO,
            high: Vec3::ZERO,
            first: 0,
            count: tree.corners.len() as u32,
        });
        let mut pending = vec![(0usize, 0usize, tree.corners.len())];
        while let Some((branch, start, end)) = pending.pop() {
            let (low, high) = tree.order[start..end].iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(low, high), triangle| {
                    let [a, b, c] = tree.corners[*triangle as usize];
                    (low.min(a).min(b).min(c), high.max(a).max(b).max(c))
                },
            );
            tree.branches[branch].low = low;
            tree.branches[branch].high = high;
            if end - start <= LEAF {
                tree.branches[branch].first = start as u32;
                tree.branches[branch].count = (end - start) as u32;
                continue;
            }
            let (spread_low, spread_high) = tree.order[start..end].iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(low, high), triangle| {
                    (
                        low.min(centres[*triangle as usize]),
                        high.max(centres[*triangle as usize]),
                    )
                },
            );
            let size = spread_high - spread_low;
            let axis = match (size.x >= size.y, size.x >= size.z, size.y >= size.z) {
                (true, true, _) => 0,
                (false, _, true) => 1,
                _ => 2,
            };
            let middle = (start + end) / 2;
            tree.order[start..end].sort_by(|a, b| {
                centres[*a as usize][axis]
                    .total_cmp(&centres[*b as usize][axis])
                    .then(a.cmp(b))
            });
            let left = tree.branches.len();
            tree.branches.push(Branch {
                low,
                high,
                first: 0,
                count: 0,
            });
            tree.branches.push(Branch {
                low,
                high,
                first: 0,
                count: 0,
            });
            tree.branches[branch].first = left as u32;
            tree.branches[branch].count = 0;
            pending.push((left, start, middle));
            pending.push((left + 1, middle, end));
        }
        tree
    }

    pub(crate) fn corners(&self, triangle: u32) -> [Vec3; 3] {
        self.corners[triangle as usize]
    }

    pub(crate) fn nearest(&self, point: Vec3, limit: f32) -> Option<Hit> {
        self.nearest_where(point, limit, |_| true)
    }

    pub(crate) fn nearest_where(
        &self,
        point: Vec3,
        limit: f32,
        keep: impl Fn(u32) -> bool,
    ) -> Option<Hit> {
        self.nearest_few_where(point, 1, limit, keep).pop()
    }

    pub(crate) fn nearest_few_where(
        &self,
        point: Vec3,
        count: usize,
        limit: f32,
        keep: impl Fn(u32) -> bool,
    ) -> Vec<Hit> {
        let mut best: Vec<Hit> = Vec::with_capacity(count + 1);
        if self.branches.is_empty() || !point.is_finite() || count == 0 {
            return best;
        }
        let mut bound = limit;
        let mut stack: Vec<(f32, u32)> = vec![(self.gap(0, point), 0)];
        while let Some((gap, branch)) = stack.pop() {
            if gap > bound {
                continue;
            }
            let entry = self.branches[branch as usize];
            if entry.count > 0 {
                for &triangle in
                    &self.order[entry.first as usize..(entry.first + entry.count) as usize]
                {
                    if !keep(triangle) {
                        continue;
                    }
                    let closest = closest_on_triangle(point, self.corners[triangle as usize]);
                    let distance = closest.point.distance(point);
                    if distance > bound {
                        continue;
                    }
                    let at = best.partition_point(|hit| {
                        hit.distance < distance
                            || (hit.distance == distance && hit.triangle < triangle)
                    });
                    best.insert(
                        at,
                        Hit {
                            triangle,
                            distance,
                            closest,
                        },
                    );
                    best.truncate(count);
                    if best.len() == count {
                        bound = best[count - 1].distance;
                    }
                }
                continue;
            }
            let (left, right) = (entry.first, entry.first + 1);
            let (near_left, near_right) = (self.gap(left, point), self.gap(right, point));
            if near_left <= near_right {
                stack.extend([(near_right, right), (near_left, left)])
            } else {
                stack.extend([(near_left, left), (near_right, right)])
            }
        }
        best
    }

    fn gap(&self, branch: u32, point: Vec3) -> f32 {
        let entry = &self.branches[branch as usize];
        (entry.low - point)
            .max(point - entry.high)
            .max(Vec3::ZERO)
            .length()
    }
}
