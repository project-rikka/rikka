mod build;
mod overlap;
mod query;
mod update;

use std::num::NonZeroUsize;

use glam::{Vec3, Vec3A};

use crate::aabb::Aabb;

pub(crate) const DEFAULT_MAX_LEAF_SIZE: u32 = 8;
const MAX_LEAF_SIZE_LIMIT: u32 = 64;

pub trait Candidate {
    fn distance(&self) -> f32;
}

impl Candidate for f32 {
    fn distance(&self) -> f32 {
        *self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BvhUpdate {
    Refitted,
    Rebuilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Settings {
    pub(crate) max_leaf_size: u32,
    pub(crate) threads: usize,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(32))]
pub(crate) struct Node {
    pub(crate) min: Vec3,
    pub(crate) start: u32,
    pub(crate) max: Vec3,
    pub(crate) count: u32,
}

impl Node {
    pub(crate) const EMPTY: Node = Node {
        min: Aabb::EMPTY.min,
        start: 0,
        max: Aabb::EMPTY.max,
        count: 0,
    };

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        self.count > 0
    }

    #[inline(always)]
    pub(crate) fn bounds(&self) -> Aabb {
        Aabb::new(self.min, self.max)
    }

    #[inline(always)]
    pub(crate) fn min_a(&self) -> Vec3A {
        Vec3A::from(self.min)
    }

    #[inline(always)]
    pub(crate) fn max_a(&self) -> Vec3A {
        Vec3A::from(self.max)
    }

    pub(crate) fn set_bounds(&mut self, bounds: &Aabb) {
        self.min = bounds.min;
        self.max = bounds.max;
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(32))]
pub(crate) struct Primitive {
    pub(crate) min: Vec3,
    pub(crate) id: u32,
    pub(crate) max: Vec3,
}

impl Primitive {
    #[inline(always)]
    pub(crate) fn bounds(&self) -> Aabb {
        Aabb::new(self.min, self.max)
    }

    #[inline(always)]
    pub(crate) fn min_a(&self) -> Vec3A {
        Vec3A::from(self.min)
    }

    #[inline(always)]
    pub(crate) fn max_a(&self) -> Vec3A {
        Vec3A::from(self.max)
    }
}

#[derive(Debug, Clone)]
pub struct Bvh {
    nodes: Vec<Node>,
    primitives: Vec<Primitive>,
    settings: Settings,
    built_cost: f32,
    links: Option<Box<update::Links>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct BvhBuilder {
    settings: Settings,
}

impl BvhBuilder {
    pub fn max_leaf_size(mut self, size: usize) -> Self {
        self.settings.max_leaf_size = (size.clamp(1, MAX_LEAF_SIZE_LIMIT as usize)) as u32;
        self
    }

    pub fn threads(mut self, threads: NonZeroUsize) -> Self {
        self.settings.threads = threads.get();
        self
    }

    pub fn build(self, bounds: &[Aabb]) -> Bvh {
        Bvh::with_settings(bounds, self.settings)
    }
}

impl Default for BvhBuilder {
    fn default() -> Self {
        let threads = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
        Self {
            settings: Settings {
                max_leaf_size: DEFAULT_MAX_LEAF_SIZE,
                threads,
            },
        }
    }
}

impl Bvh {
    pub fn new(bounds: &[Aabb]) -> Self {
        Self::builder().build(bounds)
    }

    pub fn builder() -> BvhBuilder {
        BvhBuilder::default()
    }

    fn with_settings(bounds: &[Aabb], settings: Settings) -> Self {
        assert!(
            u32::try_from(bounds.len()).is_ok(),
            "bvh primitive count exceeds u32"
        );
        let (nodes, primitives) = build::build(bounds, settings);
        let mut bvh = Self {
            nodes,
            primitives,
            settings,
            built_cost: 0.0,
            links: None,
        };
        bvh.built_cost = bvh.cost();
        bvh
    }

    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn bounds(&self) -> Aabb {
        self.nodes.first().map_or(Aabb::EMPTY, Node::bounds)
    }

    pub fn depth(&self) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let mut deepest = 0;
        let mut stack = vec![(0u32, 1usize)];
        while let Some((index, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            let node = &self.nodes[index as usize];
            if !node.is_leaf() {
                stack.push((node.start, depth + 1));
                stack.push((node.start + 1, depth + 1));
            }
        }
        deepest
    }

    pub fn cost(&self) -> f32 {
        let Some(root) = self.nodes.first() else {
            return 0.0;
        };
        let root_area = root.bounds().surface_area();
        if root_area <= 0.0 {
            return self.primitives.len() as f32;
        }
        let mut total = 0.0f64;
        let mut stack = vec![0u32];
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index as usize];
            let area = f64::from(node.bounds().surface_area());
            if node.is_leaf() {
                total += area * f64::from(node.count) * f64::from(build::INTERSECTION_COST);
            } else {
                total += area * f64::from(build::TRAVERSAL_COST);
                stack.push(node.start);
                stack.push(node.start + 1);
            }
        }
        (total / f64::from(root_area)) as f32
    }
}
