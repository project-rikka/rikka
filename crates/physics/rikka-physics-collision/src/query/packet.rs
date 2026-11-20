use rikka_physics_geometry::{Bvh4, Child, EMPTY_CHILD, Shape};
use rikka_physics_math::{Kernel, MaskW, Simd, Vec3, Wide};

use super::{QueryHit, QueryWorld};
use crate::QueryFilter;

const TINY: f32 = 1.0e-30;
const SPHERE_MARGIN: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub translation: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct Packet<W> {
    origin: [W; 3],
    translation: [W; 3],
    inverse: [W; 3],
    best: [f32; 8],
    active: u32,
}

impl<W: Wide> Packet<W> {
    #[inline(always)]
    fn new(rays: &[Ray]) -> Self {
        let lanes = |axis: usize, pick: fn(&Ray) -> Vec3| {
            W::from_fn(|lane| rays.get(lane).map_or(0.0, |ray| pick(ray)[axis]))
        };
        let inverse = |axis: usize| {
            W::from_fn(|lane| {
                let d = rays.get(lane).map_or(1.0, |ray| ray.translation[axis]);
                1.0 / if d.abs() < TINY { TINY.copysign(d) } else { d }
            })
        };
        Self {
            origin: [0, 1, 2].map(|axis| lanes(axis, |ray| ray.origin)),
            translation: [0, 1, 2].map(|axis| lanes(axis, |ray| ray.translation)),
            inverse: [inverse(0), inverse(1), inverse(2)],
            best: [1.0; 8],
            active: (1u32 << rays.len()) - 1,
        }
    }

    #[inline(always)]
    fn best(&self) -> W {
        W::load(&self.best)
    }

    #[inline(always)]
    fn slab(&self, min: [f32; 3], max: [f32; 3]) -> u32 {
        let mut near = W::default();
        let mut far = self.best();
        for axis in 0..3 {
            let t1 = (W::splat(min[axis]) - self.origin[axis]) * self.inverse[axis];
            let t2 = (W::splat(max[axis]) - self.origin[axis]) * self.inverse[axis];
            near = near.max(t1.min(t2));
            far = far.min(t1.max(t2));
        }
        near.simd_le(far).bits() & self.active
    }

    #[inline(always)]
    fn sphere(&self, center: Vec3, radius: f32) -> u32 {
        let s = [0, 1, 2].map(|axis| self.origin[axis] - W::splat(center[axis]));
        let d = self.translation;
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let sd = s[0] * d[0] + s[1] * d[1] + s[2] * d[2];
        let t = (-sd / dd.max(W::splat(TINY))).clamp(W::default(), self.best());
        let c = [0, 1, 2].map(|axis| t.mul_add(d[axis], s[axis]));
        let cc = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
        let reach = radius * (1.0 + SPHERE_MARGIN) + SPHERE_MARGIN;
        cc.simd_le(W::splat(reach * reach)).bits()
    }
}

struct Cast<'q, 'w, F> {
    world: &'q QueryWorld<'w>,
    rays: &'q [Ray],
    filter: &'q QueryFilter,
    visit: F,
}

impl<F: FnMut(usize, &QueryHit)> Kernel for Cast<'_, '_, F> {
    type Output = ();

    #[inline(always)]
    fn run<W: Wide>(mut self) {
        let mut stack = Vec::new();
        let mut hits: [Option<QueryHit>; 8] = [None; 8];
        for (chunk, rays) in self.rays.chunks(W::LANES).enumerate() {
            let mut packet = Packet::<W>::new(rays);
            hits[..rays.len()].fill(None);
            for tree in self.world.broadphase.trees() {
                traverse(
                    self.world,
                    tree.bvh(),
                    |proxy| tree.contains_in_tree(proxy),
                    rays,
                    self.filter,
                    &mut packet,
                    &mut hits,
                    &mut stack,
                );
                for &proxy in tree.pending() {
                    let bounds = self.world.broadphase.fat(proxy);
                    let mask = packet.slab(bounds.min.into(), bounds.max.into());
                    leaf(
                        self.world,
                        proxy,
                        mask,
                        rays,
                        self.filter,
                        &mut packet,
                        &mut hits,
                    );
                }
            }
            for (lane, hit) in hits[..rays.len()].iter().enumerate() {
                if let Some(hit) = hit {
                    (self.visit)(chunk * W::LANES + lane, hit);
                }
            }
        }
    }
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn traverse<W: Wide>(
    world: &QueryWorld<'_>,
    bvh: &Bvh4,
    alive: impl Fn(u32) -> bool,
    rays: &[Ray],
    filter: &QueryFilter,
    packet: &mut Packet<W>,
    hits: &mut [Option<QueryHit>; 8],
    stack: &mut Vec<(Child, u32)>,
) {
    if bvh.root().is_none() {
        return;
    }
    stack.clear();
    stack.push((Child::Node(0), packet.active));
    while let Some((child, mask)) = stack.pop() {
        let mask = mask & packet.active;
        if mask == 0 {
            continue;
        }
        match child {
            Child::Empty => {}
            Child::Leaf { start, count } => {
                for &proxy in &bvh.items()[start as usize..(start + count) as usize] {
                    if alive(proxy) {
                        leaf(world, proxy, mask, rays, filter, packet, hits);
                    }
                }
            }
            Child::Node(index) => {
                let node = bvh.node(index);
                for slot in (0..4).rev() {
                    if node.children[slot] == EMPTY_CHILD {
                        continue;
                    }
                    let min = [node.min_x[slot], node.min_y[slot], node.min_z[slot]];
                    let max = [node.max_x[slot], node.max_y[slot], node.max_z[slot]];
                    let hit = packet.slab(min, max) & mask;
                    if hit != 0 {
                        stack.push((node.child(slot), hit));
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn leaf<W: Wide>(
    world: &QueryWorld<'_>,
    proxy: u32,
    mask: u32,
    rays: &[Ray],
    filter: &QueryFilter,
    packet: &mut Packet<W>,
    hits: &mut [Option<QueryHit>; 8],
) {
    if mask == 0 {
        return;
    }
    let collider = &world.colliders[proxy as usize];
    if !world.accepts(proxy, collider, filter) {
        return;
    }
    let mut remaining = match &collider.shape {
        Shape::Sphere(sphere) => {
            let center = world.frame(collider).apply(sphere.center);
            packet.sphere(center, sphere.radius) & mask
        }
        _ => mask,
    };
    while remaining != 0 {
        let lane = remaining.trailing_zeros() as usize;
        remaining &= remaining - 1;
        let ray = rays[lane];
        let best = packet.best[lane];
        if let Some(hit) = world.ray_shape(proxy, collider, ray.origin, ray.translation, best) {
            packet.best[lane] = hit.fraction;
            hits[lane] = Some(hit);
        }
    }
}

impl QueryWorld<'_> {
    pub fn cast_rays_closest(
        &self,
        simd: Simd,
        rays: &[Ray],
        filter: &QueryFilter,
        visit: impl FnMut(usize, &QueryHit),
    ) {
        simd.run(Cast {
            world: self,
            rays,
            filter,
            visit,
        });
    }
}
