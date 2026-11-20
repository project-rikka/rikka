use std::ops::ControlFlow;

use glam::{Vec3, Vec3A};
use smallvec::SmallVec;

use super::{Bvh, Candidate, Node, Primitive};
use crate::aabb::Aabb;
use crate::plane::{Containment, Frustum, Plane};
use crate::ray::{Ray, RaySlab};

type Stack<T> = SmallVec<[T; 64]>;

struct Best<C> {
    candidate: Option<C>,
    primitive: u32,
    limit: f32,
}

impl<C: Candidate> Best<C> {
    fn new(limit: f32) -> Self {
        Self {
            candidate: None,
            primitive: u32::MAX,
            limit,
        }
    }

    fn offer(&mut self, primitive: u32, candidate: C) {
        let distance = candidate.distance();
        if distance.is_nan() || distance > self.limit {
            return;
        }
        if distance < self.limit || self.candidate.is_none() || primitive < self.primitive {
            self.limit = distance;
            self.primitive = primitive;
            self.candidate = Some(candidate);
        }
    }
}

impl Bvh {
    pub fn ray_cast<C: Candidate>(
        &self,
        ray: &Ray,
        max_distance: f32,
        test: impl FnMut(u32, f32) -> Option<C>,
    ) -> Option<C> {
        self.cast(ray, 0.0, max_distance, test)
    }

    pub fn sphere_cast<C: Candidate>(
        &self,
        ray: &Ray,
        radius: f32,
        max_distance: f32,
        test: impl FnMut(u32, f32) -> Option<C>,
    ) -> Option<C> {
        self.cast(ray, radius.max(0.0), max_distance, test)
    }

    fn cast<C: Candidate>(
        &self,
        ray: &Ray,
        radius: f32,
        max_distance: f32,
        mut test: impl FnMut(u32, f32) -> Option<C>,
    ) -> Option<C> {
        let root = self.nodes.first()?;
        let slab = RaySlab::new(ray);
        let margin = Vec3A::splat(radius);
        let enter = |min: Vec3A, max: Vec3A, limit: f32| {
            slab.interval(min - margin, max + margin, 0.0, limit)
        };
        let mut best = Best::new(max_distance);
        let mut stack: Stack<(u32, f32)> = SmallVec::new();
        stack.push((0, enter(root.min_a(), root.max_a(), max_distance)?));
        while let Some((index, distance)) = stack.pop() {
            if distance > best.limit {
                continue;
            }
            let node = &self.nodes[index as usize];
            if node.is_leaf() {
                for primitive in self.leaf(node) {
                    if enter(primitive.min_a(), primitive.max_a(), best.limit).is_some()
                        && let Some(candidate) = test(primitive.id, best.limit)
                    {
                        best.offer(primitive.id, candidate);
                    }
                }
                continue;
            }
            let (left, right) = self.children(node);
            let near = enter(left.min_a(), left.max_a(), best.limit);
            let far = enter(right.min_a(), right.max_a(), best.limit);
            push_ordered(&mut stack, node.start, near, far);
        }
        best.candidate
    }

    pub fn ray_cast_any(
        &self,
        ray: &Ray,
        max_distance: f32,
        mut test: impl FnMut(u32, f32) -> bool,
    ) -> bool {
        let mut hit = false;
        self.visit_ray(ray, max_distance, |primitive| {
            hit = test(primitive, max_distance);
            match hit {
                true => ControlFlow::Break(()),
                false => ControlFlow::Continue(()),
            }
        });
        hit
    }

    pub fn ray_cast_all(&self, ray: &Ray, max_distance: f32, mut visit: impl FnMut(u32)) {
        self.visit_ray(ray, max_distance, |primitive| {
            visit(primitive);
            ControlFlow::Continue(())
        });
    }

    fn visit_ray(
        &self,
        ray: &Ray,
        max_distance: f32,
        mut visit: impl FnMut(u32) -> ControlFlow<()>,
    ) {
        if self.nodes.is_empty() {
            return;
        }
        let slab = RaySlab::new(ray);
        let enter = |min: Vec3A, max: Vec3A| slab.interval(min, max, 0.0, max_distance);
        let mut stack: Stack<u32> = SmallVec::new();
        stack.push(0);
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index as usize];
            if enter(node.min_a(), node.max_a()).is_none() {
                continue;
            }
            if node.is_leaf() {
                for primitive in self.leaf(node) {
                    if enter(primitive.min_a(), primitive.max_a()).is_some()
                        && visit(primitive.id).is_break()
                    {
                        return;
                    }
                }
                continue;
            }
            stack.push(node.start + 1);
            stack.push(node.start);
        }
    }

    pub fn nearest<C: Candidate>(
        &self,
        max_distance: f32,
        mut bound: impl FnMut(&Aabb, f32) -> f32,
        mut test: impl FnMut(u32, f32) -> Option<C>,
    ) -> Option<C> {
        let root = self.nodes.first()?;
        let mut best = Best::new(max_distance);
        let root_bound = bound(&root.bounds(), best.limit);
        if root_bound > best.limit {
            return None;
        }
        let mut stack: Stack<(u32, f32)> = SmallVec::new();
        stack.push((0, root_bound));
        while let Some((index, lower)) = stack.pop() {
            if lower > best.limit {
                continue;
            }
            let node = &self.nodes[index as usize];
            if node.is_leaf() {
                for primitive in self.leaf(node) {
                    if bound(&primitive.bounds(), best.limit) <= best.limit
                        && let Some(candidate) = test(primitive.id, best.limit)
                    {
                        best.offer(primitive.id, candidate);
                    }
                }
                continue;
            }
            let (left, right) = self.children(node);
            let left_bound = bound(&left.bounds(), best.limit);
            let right_bound = bound(&right.bounds(), best.limit);
            let accept = |value: f32| (value <= best.limit).then_some(value);
            push_ordered(
                &mut stack,
                node.start,
                accept(left_bound),
                accept(right_bound),
            );
        }
        best.candidate
    }

    pub fn nearest_to_point<C: Candidate>(
        &self,
        point: Vec3,
        max_distance: f32,
        test: impl FnMut(u32, f32) -> Option<C>,
    ) -> Option<C> {
        self.nearest(
            max_distance,
            |bounds, _| bounds.distance_squared_to_point(point).sqrt(),
            test,
        )
    }

    pub fn for_each_in_aabb(&self, bounds: &Aabb, visit: impl FnMut(u32)) {
        self.visit_where(|node| node.intersects(bounds), visit);
    }

    pub fn for_each_in_sphere(&self, center: Vec3, radius: f32, visit: impl FnMut(u32)) {
        self.visit_where(|node| node.intersects_sphere(center, radius), visit);
    }

    pub fn for_each_crossing_plane(&self, plane: &Plane, visit: impl FnMut(u32)) {
        let crosses = |bounds: &Aabb| {
            let ascending = plane.normal.cmpge(Vec3::ZERO);
            plane.signed_distance(Vec3::select(ascending, bounds.max, bounds.min)) >= 0.0
                && plane.signed_distance(Vec3::select(ascending, bounds.min, bounds.max)) <= 0.0
        };
        self.visit_where(crosses, visit);
    }

    fn visit_where(&self, accept: impl Fn(&Aabb) -> bool, mut visit: impl FnMut(u32)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack: Stack<u32> = SmallVec::new();
        stack.push(0);
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index as usize];
            if !accept(&node.bounds()) {
                continue;
            }
            if node.is_leaf() {
                for primitive in self.leaf(node) {
                    if accept(&primitive.bounds()) {
                        visit(primitive.id);
                    }
                }
                continue;
            }
            stack.push(node.start + 1);
            stack.push(node.start);
        }
    }

    pub fn for_each_in_frustum(&self, frustum: &Frustum, mut visit: impl FnMut(u32, Containment)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack: Stack<(u32, bool)> = SmallVec::new();
        stack.push((0, false));
        while let Some((index, inside)) = stack.pop() {
            let node = &self.nodes[index as usize];
            let inside = inside
                || match frustum.classify_aabb(&node.bounds()) {
                    Containment::Outside => continue,
                    Containment::Inside => true,
                    Containment::Intersecting => false,
                };
            if node.is_leaf() {
                for primitive in self.leaf(node) {
                    let containment = match inside {
                        true => Containment::Inside,
                        false => frustum.classify_aabb(&primitive.bounds()),
                    };
                    if containment != Containment::Outside && !primitive.bounds().is_empty() {
                        visit(primitive.id, containment);
                    }
                }
                continue;
            }
            stack.push((node.start + 1, inside));
            stack.push((node.start, inside));
        }
    }

    pub fn traverse(
        &self,
        mut enter: impl FnMut(&Aabb) -> Option<f32>,
        mut visit: impl FnMut(u32) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        let Some(root) = self.nodes.first() else {
            return ControlFlow::Continue(());
        };
        if enter(&root.bounds()).is_none() {
            return ControlFlow::Continue(());
        }
        let mut stack: Stack<u32> = SmallVec::new();
        stack.push(0);
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index as usize];
            if node.is_leaf() {
                for primitive in self.leaf(node) {
                    visit(primitive.id)?;
                }
                continue;
            }
            let (left, right) = self.children(node);
            let left_key = enter(&left.bounds());
            let right_key = enter(&right.bounds());
            match (left_key, right_key) {
                (Some(left_key), Some(right_key)) if right_key < left_key => {
                    stack.push(node.start);
                    stack.push(node.start + 1);
                }
                (Some(_), Some(_)) => {
                    stack.push(node.start + 1);
                    stack.push(node.start);
                }
                (Some(_), None) => stack.push(node.start),
                (None, Some(_)) => stack.push(node.start + 1),
                (None, None) => {}
            }
        }
        ControlFlow::Continue(())
    }

    #[inline(always)]
    pub(crate) fn leaf(&self, node: &Node) -> &[Primitive] {
        &self.primitives[node.start as usize..(node.start + node.count) as usize]
    }

    #[inline(always)]
    pub(crate) fn children(&self, node: &Node) -> (&Node, &Node) {
        (
            &self.nodes[node.start as usize],
            &self.nodes[node.start as usize + 1],
        )
    }
}

#[inline(always)]
fn push_ordered(
    stack: &mut Stack<(u32, f32)>,
    first_child: u32,
    left: Option<f32>,
    right: Option<f32>,
) {
    match (left, right) {
        (Some(left), Some(right)) if right < left => {
            stack.push((first_child, left));
            stack.push((first_child + 1, right));
        }
        (Some(left), Some(right)) => {
            stack.push((first_child + 1, right));
            stack.push((first_child, left));
        }
        (Some(left), None) => stack.push((first_child, left)),
        (None, Some(right)) => stack.push((first_child + 1, right)),
        (None, None) => {}
    }
}
