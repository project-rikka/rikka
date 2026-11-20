use glam::Vec3;

const TREE_TYPE: usize = 2;
const NO_CHILD: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
struct Node {
    bounds: [f32; 6],
    children: [u32; TREE_TYPE],
    child_count: u8,
    main_axis: u8,
    primitive: u32,
}

impl Node {
    fn empty() -> Node {
        Node {
            bounds: [
                f32::MAX,
                -f32::MAX,
                f32::MAX,
                -f32::MAX,
                f32::MAX,
                -f32::MAX,
            ],
            children: [NO_CHILD; TREE_TYPE],
            child_count: 0,
            main_axis: 0,
            primitive: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct KdopHit {
    pub(crate) primitive: Option<u32>,
    pub(crate) distance: f32,
    pub(crate) point: Vec3,
}

pub(crate) struct KdopRay {
    pub(crate) origin: Vec3,
    pub(crate) direction: Vec3,
    pub(crate) radius: f32,
}

#[derive(Debug)]
pub(crate) struct KdopTree {
    nodes: Vec<Node>,
    root: Option<u32>,
}

struct Helper {
    leafs: usize,
    leafs_per_child: [usize; 32],
    branches_on_level: [usize; 32],
    remain_leafs: usize,
}

impl Helper {
    fn new(leafs: usize) -> Helper {
        let mut helper = Helper {
            leafs,
            leafs_per_child: [0; 32],
            branches_on_level: [0; 32],
            remain_leafs: 0,
        };
        helper.leafs_per_child[0] = 1;
        while helper.leafs_per_child[0] < leafs {
            helper.leafs_per_child[0] *= TREE_TYPE;
        }
        helper.branches_on_level[0] = 1;
        let mut depth = 1;
        while depth < 32 && helper.leafs_per_child[depth - 1] != 0 {
            helper.branches_on_level[depth] = helper.branches_on_level[depth - 1] * TREE_TYPE;
            helper.leafs_per_child[depth] = helper.leafs_per_child[depth - 1] / TREE_TYPE;
            depth += 1;
        }
        let remain = leafs as isize - helper.leafs_per_child[1] as isize;
        let nodes = (remain + TREE_TYPE as isize - 2) / (TREE_TYPE as isize - 1);
        helper.remain_leafs = (remain + nodes).max(0) as usize;
        helper
    }

    fn leafs_index(&self, depth: usize, child: usize) -> usize {
        let min_leaf = child * self.leafs_per_child[depth - 1];
        if min_leaf <= self.remain_leafs {
            return min_leaf;
        }
        if self.leafs_per_child[depth] != 0 {
            return self.leafs
                - (self.branches_on_level[depth - 1] - child) * self.leafs_per_child[depth];
        }
        self.remain_leafs
    }
}

fn needed_branches(leafs: usize) -> usize {
    1.max((leafs + TREE_TYPE - 3) / (TREE_TYPE - 1))
}

fn largest_axis(bounds: &[f32; 6]) -> usize {
    let x = bounds[1] - bounds[0];
    let y = bounds[3] - bounds[2];
    let z = bounds[5] - bounds[4];
    if x > y {
        if x > z { 1 } else { 5 }
    } else if y > z {
        3
    } else {
        5
    }
}

impl KdopTree {
    pub(crate) fn new(primitives: impl Iterator<Item = (u32, Vec<Vec3>)>) -> KdopTree {
        let mut nodes: Vec<Node> = Vec::new();
        for (primitive, points) in primitives {
            let mut node = Node::empty();
            node.primitive = primitive;
            for point in points {
                for axis in 0..3 {
                    node.bounds[axis * 2] = point[axis].min(node.bounds[axis * 2]);
                    node.bounds[axis * 2 + 1] = point[axis].max(node.bounds[axis * 2 + 1]);
                }
            }
            for axis in 0..3 {
                node.bounds[axis * 2] -= f32::EPSILON;
                node.bounds[axis * 2 + 1] += f32::EPSILON;
            }
            nodes.push(node);
        }
        let leafs = nodes.len();
        if leafs == 0 {
            return KdopTree { nodes, root: None };
        }
        let branches = needed_branches(leafs);
        let branch_base = nodes.len();
        nodes.extend(std::iter::repeat_n(Node::empty(), branches + 1));
        let branch = |index: usize| (branch_base + index) as u32;
        let mut order: Vec<u32> = (0..leafs as u32).collect();
        let mut tree = KdopTree {
            nodes,
            root: Some(branch(1)),
        };
        if leafs == 1 {
            let root = branch(1) as usize;
            tree.refit(root, &order, 0, 1);
            tree.nodes[root].main_axis = (largest_axis(&tree.nodes[root].bounds) / 2) as u8;
            tree.nodes[root].child_count = 1;
            tree.nodes[root].children[0] = order[0];
            return tree;
        }
        let helper = Helper::new(leafs);
        let mut level = 1usize;
        let mut depth = 1usize;
        while level <= branches {
            let first_of_next_level = level * TREE_TYPE;
            let stop = first_of_next_level.min(branches + 1);
            for parent_index in level..stop {
                let parent = branch(parent_index) as usize;
                let begin = helper.leafs_index(depth, parent_index - level);
                let end = helper.leafs_index(depth, parent_index - level + 1);
                tree.refit(parent, &order, begin, end);
                let split_axis = largest_axis(&tree.nodes[parent].bounds);
                tree.nodes[parent].main_axis = (split_axis / 2) as u8;
                let mut nth = [0usize; TREE_TYPE + 1];
                nth[0] = begin;
                nth[TREE_TYPE] = end;
                for (k, slot) in nth.iter_mut().enumerate().take(TREE_TYPE).skip(1) {
                    let child = parent_index * TREE_TYPE + k;
                    *slot = helper.leafs_index(depth + 1, child - first_of_next_level);
                }
                for k in 0..TREE_TYPE - 1 {
                    if nth[k] >= nth[TREE_TYPE] {
                        break;
                    }
                    tree.partition_nth(&mut order, nth[k], nth[TREE_TYPE], nth[k + 1], split_axis);
                }
                let mut children = [NO_CHILD; TREE_TYPE];
                let mut count = 0;
                for (k, slot) in children.iter_mut().enumerate() {
                    let child = parent_index * TREE_TYPE + k;
                    let child_level = child - first_of_next_level;
                    let child_begin = helper.leafs_index(depth + 1, child_level);
                    let child_end = helper.leafs_index(depth + 1, child_level + 1);
                    *slot = match child_end.saturating_sub(child_begin) {
                        0 => break,
                        1 => order[child_begin],
                        _ => branch(child),
                    };
                    count = k + 1;
                }
                tree.nodes[parent].children = children;
                tree.nodes[parent].child_count = count as u8;
            }
            level = first_of_next_level;
            depth += 1;
        }
        tree
    }

    fn refit(&mut self, node: usize, order: &[u32], begin: usize, end: usize) {
        let mut bounds = Node::empty().bounds;
        for &leaf in &order[begin..end] {
            let leaf_bounds = self.nodes[leaf as usize].bounds;
            for axis in 0..3 {
                bounds[axis * 2] = leaf_bounds[axis * 2].min(bounds[axis * 2]);
                bounds[axis * 2 + 1] = leaf_bounds[axis * 2 + 1].max(bounds[axis * 2 + 1]);
            }
        }
        self.nodes[node].bounds = bounds;
    }

    fn key(&self, order: &[u32], slot: usize, axis: usize) -> f32 {
        self.nodes[order[slot] as usize].bounds[axis]
    }

    fn partition_nth(
        &self,
        order: &mut [u32],
        mut begin: usize,
        mut end: usize,
        n: usize,
        axis: usize,
    ) {
        while end - begin > 3 {
            let pivot = self.median_of_three(order, begin, (begin + end) / 2, end - 1, axis);
            let cut = self.partition(order, begin, end, pivot, axis);
            if cut <= n {
                begin = cut;
            } else {
                end = cut;
            }
        }
        for i in begin..end {
            let mut j = i;
            let moving = order[i];
            let value = self.nodes[moving as usize].bounds[axis];
            while j != begin && value < self.key(order, j - 1, axis) {
                order[j] = order[j - 1];
                j -= 1;
            }
            order[j] = moving;
        }
    }

    fn median_of_three(&self, order: &[u32], lo: usize, mid: usize, hi: usize, axis: usize) -> f32 {
        let (a, b, c) = (
            self.key(order, lo, axis),
            self.key(order, mid, axis),
            self.key(order, hi, axis),
        );
        if b < a {
            if c < b {
                return b;
            }
            if c < a {
                return c;
            }
            return a;
        }
        if c < b {
            if c < a {
                return a;
            }
            return c;
        }
        b
    }

    fn partition(&self, order: &mut [u32], lo: usize, hi: usize, pivot: f32, axis: usize) -> usize {
        let (mut i, mut j) = (lo, hi);
        loop {
            while self.key(order, i, axis) < pivot {
                i += 1;
            }
            j -= 1;
            while pivot < self.key(order, j, axis) {
                j -= 1;
            }
            if i >= j {
                return i;
            }
            order.swap(i, j);
            i += 1;
        }
    }

    pub(crate) fn ray_cast(
        &self,
        ray: &KdopRay,
        hit: &mut KdopHit,
        test: &mut impl FnMut(u32, &KdopRay, &mut KdopHit),
    ) {
        let Some(root) = self.root else {
            return;
        };
        let mut along = [0.0f32; 3];
        let mut inverse = [0.0f32; 3];
        let mut index = [0usize; 6];
        for axis in 0..3 {
            along[axis] = ray.direction[axis];
            if along[axis].abs() < f32::EPSILON {
                along[axis] = 0.0;
                inverse[axis] = f32::MAX;
            } else {
                inverse[axis] = 1.0 / along[axis];
            }
            index[2 * axis] = usize::from(inverse[axis] < 0.0) + 2 * axis;
            index[2 * axis + 1] = 1 - usize::from(inverse[axis] < 0.0) + 2 * axis;
        }
        let cast = Cast {
            tree: self,
            ray,
            along,
            inverse,
            index,
        };
        cast.visit(root, hit, test);
    }
}

struct Cast<'a> {
    tree: &'a KdopTree,
    ray: &'a KdopRay,
    along: [f32; 3],
    inverse: [f32; 3],
    index: [usize; 6],
}

impl Cast<'_> {
    fn nearest(&self, bounds: &[f32; 6], limit: f32) -> f32 {
        let ray = self.ray;
        let mut low = 0.0f32;
        let mut upper = limit;
        for axis in 0..3 {
            let (min, max) = (bounds[axis * 2], bounds[axis * 2 + 1]);
            if self.along[axis] == 0.0 {
                if ray.origin[axis] < min - ray.radius || ray.origin[axis] > max + ray.radius {
                    return f32::MAX;
                }
            } else {
                let ll = (min - ray.radius - ray.origin[axis]) / self.along[axis];
                let lu = (max + ray.radius - ray.origin[axis]) / self.along[axis];
                if self.along[axis] > 0.0 {
                    low = ll.max(low);
                    upper = lu.min(upper);
                } else {
                    low = lu.max(low);
                    upper = ll.min(upper);
                }
                if low > upper {
                    return f32::MAX;
                }
            }
        }
        low
    }

    fn fast_nearest(&self, bounds: &[f32; 6], limit: f32) -> f32 {
        let origin = self.ray.origin;
        let t1x = (bounds[self.index[0]] - origin.x) * self.inverse[0];
        let t2x = (bounds[self.index[1]] - origin.x) * self.inverse[0];
        let t1y = (bounds[self.index[2]] - origin.y) * self.inverse[1];
        let t2y = (bounds[self.index[3]] - origin.y) * self.inverse[1];
        let t1z = (bounds[self.index[4]] - origin.z) * self.inverse[2];
        let t2z = (bounds[self.index[5]] - origin.z) * self.inverse[2];
        if (t1x > t2y || t2x < t1y || t1x > t2z || t2x < t1z || t1y > t2z || t2y < t1z)
            || (t2x < 0.0 || t2y < 0.0 || t2z < 0.0)
            || (t1x > limit || t1y > limit || t1z > limit)
        {
            return f32::MAX;
        }
        t1x.max(t1y).max(t1z)
    }

    fn visit(
        &self,
        node: u32,
        hit: &mut KdopHit,
        test: &mut impl FnMut(u32, &KdopRay, &mut KdopHit),
    ) {
        let record = &self.tree.nodes[node as usize];
        let distance = match self.ray.radius == 0.0 {
            true => self.fast_nearest(&record.bounds, hit.distance),
            false => self.nearest(&record.bounds, hit.distance),
        };
        if distance >= hit.distance {
            return;
        }
        if record.child_count == 0 {
            test(record.primitive, self.ray, hit);
            return;
        }
        let children = &record.children[..record.child_count as usize];
        match self.along[record.main_axis as usize] > 0.0 {
            true => {
                for &child in children {
                    self.visit(child, hit, test);
                }
            }
            false => {
                for &child in children.iter().rev() {
                    self.visit(child, hit, test);
                }
            }
        }
    }
}
