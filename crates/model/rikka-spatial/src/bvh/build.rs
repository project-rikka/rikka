use glam::Vec3;

use super::{Node, Primitive, Settings};
use crate::aabb::Aabb;

pub(crate) const TRAVERSAL_COST: f32 = 1.0;
pub(crate) const INTERSECTION_COST: f32 = 1.0;
const BINS: usize = 16;
const MAX_SAH_DEPTH: usize = 64;
const PARALLEL_MIN_ITEMS: usize = 16 * 1024;

#[derive(Clone, Copy)]
struct Item {
    min: Vec3,
    max: Vec3,
    centroid: Vec3,
    id: u32,
}

#[derive(Clone, Copy)]
struct Extent {
    bounds: Aabb,
    centroids: Aabb,
}

impl Extent {
    const EMPTY: Extent = Extent {
        bounds: Aabb::EMPTY,
        centroids: Aabb::EMPTY,
    };

    fn of(items: &[Item]) -> Self {
        items
            .iter()
            .fold(Self::EMPTY, |extent, item| extent.with(item))
    }

    fn with(&self, item: &Item) -> Self {
        Extent {
            bounds: Aabb::new(self.bounds.min.min(item.min), self.bounds.max.max(item.max)),
            centroids: self.centroids.with_point(item.centroid),
        }
    }

    fn union(&self, other: &Extent) -> Self {
        Extent {
            bounds: self.bounds.union(&other.bounds),
            centroids: self.centroids.union(&other.centroids),
        }
    }
}

#[derive(Clone, Copy)]
struct Bin {
    extent: Extent,
    count: u32,
}

struct Split {
    axis: usize,
    boundary: usize,
    left: Extent,
    right: Extent,
    left_count: usize,
    cost: f32,
}

struct Context {
    max_leaf_size: usize,
    parallel_depth: usize,
}

pub(super) fn sanitize(bounds: &Aabb) -> Aabb {
    let valid = bounds.min.is_finite() && bounds.max.is_finite() && !bounds.is_empty();
    match valid {
        true => *bounds,
        false => Aabb::EMPTY,
    }
}

pub(super) fn build(bounds: &[Aabb], settings: Settings) -> (Vec<Node>, Vec<Primitive>) {
    if bounds.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut items: Vec<Item> = bounds
        .iter()
        .enumerate()
        .map(|(id, bounds)| {
            let bounds = sanitize(bounds);
            let centroid = match bounds.is_empty() {
                true => Vec3::ZERO,
                false => bounds.center(),
            };
            Item {
                min: bounds.min,
                max: bounds.max,
                centroid,
                id: id as u32,
            }
        })
        .collect();
    let context = Context {
        max_leaf_size: settings.max_leaf_size as usize,
        parallel_depth: match settings.threads > 1 {
            true => settings.threads.next_power_of_two().trailing_zeros() as usize + 1,
            false => 0,
        },
    };
    let extent = Extent::of(&items);
    let nodes = build_subtree(&mut items, 0, extent, 0, &context);
    let primitives = items
        .iter()
        .map(|item| Primitive {
            min: item.min,
            id: item.id,
            max: item.max,
        })
        .collect();
    (nodes, primitives)
}

pub(super) fn rebuild_primitives(
    primitives: &[Primitive],
    settings: Settings,
) -> (Vec<Node>, Vec<Primitive>) {
    let mut bounds = vec![Aabb::EMPTY; primitives.len()];
    for primitive in primitives {
        bounds[primitive.id as usize] = primitive.bounds();
    }
    build(&bounds, settings)
}

fn build_subtree(
    items: &mut [Item],
    offset: usize,
    extent: Extent,
    depth: usize,
    context: &Context,
) -> Vec<Node> {
    if depth < context.parallel_depth && items.len() >= PARALLEL_MIN_ITEMS {
        if let Some((left_extent, right_extent, left_count)) =
            choose_split(items, &extent, depth, context)
        {
            let (left_items, right_items) = items.split_at_mut(left_count);
            let (left, right) = std::thread::scope(|scope| {
                let left = std::thread::Builder::new()
                    .name("rikka-bvh-build".into())
                    .spawn_scoped(scope, || {
                        build_subtree(left_items, offset, left_extent, depth + 1, context)
                    })
                    .expect("failed to spawn bvh build thread");
                let right = build_subtree(
                    right_items,
                    offset + left_count,
                    right_extent,
                    depth + 1,
                    context,
                );
                (left.join().expect("bvh build thread panicked"), right)
            });
            return merge(&extent.bounds, left, right);
        }
        return vec![leaf(&extent.bounds, offset, items.len())];
    }
    let mut nodes = Vec::with_capacity(2 * items.len().div_ceil(context.max_leaf_size.max(1)) + 1);
    nodes.push(Node::EMPTY);
    build_into(&mut nodes, 0, items, offset, extent, depth, context);
    nodes
}

fn merge(bounds: &Aabb, left: Vec<Node>, right: Vec<Node>) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(1 + left.len() + right.len());
    let mut root = Node::EMPTY;
    root.set_bounds(bounds);
    root.start = 1;
    root.count = 0;
    nodes.push(root);
    let left_len = left.len();
    let remap_left = |index: u32| index + 2;
    let remap_right = |index: u32| index + left_len as u32 + 1;
    let relink = |node: Node, remap: &dyn Fn(u32) -> u32| match node.is_leaf() {
        true => node,
        false => Node {
            start: remap(node.start),
            ..node
        },
    };
    nodes.push(relink(left[0], &remap_left));
    nodes.push(relink(right[0], &remap_right));
    nodes.extend(left[1..].iter().map(|node| relink(*node, &remap_left)));
    nodes.extend(right[1..].iter().map(|node| relink(*node, &remap_right)));
    nodes
}

fn leaf(bounds: &Aabb, offset: usize, count: usize) -> Node {
    let mut node = Node::EMPTY;
    node.set_bounds(bounds);
    node.start = offset as u32;
    node.count = count as u32;
    node
}

fn build_into(
    nodes: &mut Vec<Node>,
    slot: usize,
    items: &mut [Item],
    offset: usize,
    extent: Extent,
    depth: usize,
    context: &Context,
) {
    let Some((left_extent, right_extent, left_count)) =
        choose_split(items, &extent, depth, context)
    else {
        nodes[slot] = leaf(&extent.bounds, offset, items.len());
        return;
    };
    let children = nodes.len();
    nodes.push(Node::EMPTY);
    nodes.push(Node::EMPTY);
    let mut node = Node::EMPTY;
    node.set_bounds(&extent.bounds);
    node.start = children as u32;
    nodes[slot] = node;
    let (left_items, right_items) = items.split_at_mut(left_count);
    build_into(
        nodes,
        children,
        left_items,
        offset,
        left_extent,
        depth + 1,
        context,
    );
    build_into(
        nodes,
        children + 1,
        right_items,
        offset + left_count,
        right_extent,
        depth + 1,
        context,
    );
}

fn choose_split(
    items: &mut [Item],
    extent: &Extent,
    depth: usize,
    context: &Context,
) -> Option<(Extent, Extent, usize)> {
    let count = items.len();
    if count <= 1 {
        return None;
    }
    let leaf_cost = extent.bounds.surface_area() * count as f32 * INTERSECTION_COST;
    if depth >= MAX_SAH_DEPTH {
        if count <= context.max_leaf_size {
            return None;
        }
        return Some(median_split(items, extent));
    }
    match find_sah_split(items, extent) {
        Some(split) => {
            let split_cost = TRAVERSAL_COST * extent.bounds.surface_area() + split.cost;
            if count <= context.max_leaf_size && leaf_cost <= split_cost {
                return None;
            }
            let left_count = partition(items, |item| {
                bin_index(item.centroid[split.axis], extent, split.axis) < split.boundary
            });
            debug_assert_eq!(left_count, split.left_count);
            Some((split.left, split.right, left_count))
        }
        None if count <= context.max_leaf_size => None,
        None => Some(median_split(items, extent)),
    }
}

fn bin_index(value: f32, extent: &Extent, axis: usize) -> usize {
    let low = extent.centroids.min[axis];
    let span = extent.centroids.max[axis] - low;
    let scaled = (value - low) * (BINS as f32 / span);
    (scaled as usize).min(BINS - 1)
}

fn find_sah_split(items: &[Item], extent: &Extent) -> Option<Split> {
    let span = extent.centroids.size();
    let empty = Bin {
        extent: Extent::EMPTY,
        count: 0,
    };
    let mut bins = [[empty; BINS]; 3];
    let axes: Vec<usize> = (0..3)
        .filter(|&axis| span[axis] > 0.0 && span[axis].is_finite())
        .collect();
    if axes.is_empty() {
        return None;
    }
    let scales = Vec3::from_array([0, 1, 2].map(|axis| match span[axis] > 0.0 {
        true => BINS as f32 / span[axis],
        false => 0.0,
    }));
    let low = extent.centroids.min;
    for item in items {
        let scaled = (item.centroid - low) * scales;
        for &axis in &axes {
            let index = (scaled[axis] as usize).min(BINS - 1);
            let bin = &mut bins[axis][index];
            bin.extent = bin.extent.with(item);
            bin.count += 1;
        }
    }
    let mut best: Option<Split> = None;
    for &axis in &axes {
        let bins = &bins[axis];
        let mut right_areas = [0.0f32; BINS];
        let mut right_extents = [Extent::EMPTY; BINS];
        let mut accumulated = Extent::EMPTY;
        for index in (1..BINS).rev() {
            accumulated = accumulated.union(&bins[index].extent);
            right_extents[index] = accumulated;
            right_areas[index] = accumulated.bounds.surface_area();
        }
        let mut left = Extent::EMPTY;
        let mut left_count = 0usize;
        for boundary in 1..BINS {
            left = left.union(&bins[boundary - 1].extent);
            left_count += bins[boundary - 1].count as usize;
            let right_count = items.len() - left_count;
            if left_count == 0 || right_count == 0 {
                continue;
            }
            let cost = INTERSECTION_COST
                * (left.bounds.surface_area() * left_count as f32
                    + right_areas[boundary] * right_count as f32);
            if best.as_ref().is_none_or(|split| cost < split.cost) {
                best = Some(Split {
                    axis,
                    boundary,
                    left,
                    right: right_extents[boundary],
                    left_count,
                    cost,
                });
            }
        }
    }
    best
}

fn median_split(items: &mut [Item], extent: &Extent) -> (Extent, Extent, usize) {
    let span = extent.centroids.size();
    let axis = if span.x >= span.y && span.x >= span.z {
        0
    } else if span.y >= span.z {
        1
    } else {
        2
    };
    let middle = items.len() / 2;
    items.select_nth_unstable_by(middle, |left, right| {
        left.centroid[axis]
            .total_cmp(&right.centroid[axis])
            .then(left.id.cmp(&right.id))
    });
    let (left, right) = items.split_at(middle);
    (Extent::of(left), Extent::of(right), middle)
}

fn partition(items: &mut [Item], goes_left: impl Fn(&Item) -> bool) -> usize {
    let mut left = 0;
    let mut right = items.len();
    loop {
        while left < right && goes_left(&items[left]) {
            left += 1;
        }
        while left < right && !goes_left(&items[right - 1]) {
            right -= 1;
        }
        if left >= right {
            return left;
        }
        items.swap(left, right - 1);
        left += 1;
        right -= 1;
    }
}
