use rikka_physics_core::BitSet;
use rikka_physics_geometry::{Manifold, MeshContactScratch, Tolerance, collide_triangulated};
use rikka_physics_math::Mat3;

use super::context::KernelContext;
use super::finish::{finish_many, trim_unreachable};
use super::recycle::Refresh;
use crate::ContactManifold;
use crate::contact::{ContactHeader, ExtraManifolds};

#[derive(Debug)]
pub(crate) struct TriangulatedScratch {
    mesh: MeshContactScratch,
    manifolds: Vec<Manifold>,
    previous: Vec<ContactManifold>,
}

impl TriangulatedScratch {
    pub(crate) fn new(tolerance: Tolerance) -> Self {
        Self {
            mesh: MeshContactScratch::new(tolerance),
            manifolds: Vec::new(),
            previous: Vec::new(),
        }
    }
}

fn previous_extras<'a>(ctx: &KernelContext<'a>, header: &ContactHeader) -> &'a [ContactManifold] {
    let extra = header.extra;
    if extra.count == 0 || extra.stamp != ctx.stamp.wrapping_sub(1) {
        return &[];
    }
    let pool = unsafe { ctx.previous[extra.worker as usize].shared() };
    unsafe { pool.slice(extra.start as usize, (extra.start + extra.count) as usize) }
}

pub(crate) fn carry_extras(
    ctx: &KernelContext<'_>,
    header: &mut ContactHeader,
    worker: u32,
    refresh: &Refresh,
) {
    let extras = previous_extras(ctx, header);
    if extras.is_empty() {
        header.extra = ExtraManifolds::default();
        return;
    }
    let mut pool = ctx.current.local(worker);
    let start = pool.len() as u32;
    for manifold in extras {
        let mut manifold = *manifold;
        refresh.apply(&mut manifold);
        pool.push(manifold);
    }
    header.extra = ExtraManifolds {
        worker,
        start,
        count: extras.len() as u32,
        stamp: ctx.stamp,
    };
}

pub(crate) fn collide_one(id: u32, ctx: &KernelContext<'_>, worker: u32, changed: &mut BitSet) {
    let header = ctx.header(id);
    let contact = ctx.contact(id);
    let (shape_a, shape_b) = ctx.shapes(header);
    let (frame_a, frame_b) = ctx.frames(header);
    let mut scratch = ctx.scratch.local(worker);
    let TriangulatedScratch {
        mesh,
        manifolds,
        previous,
    } = &mut *scratch;
    previous.clear();
    previous.push(contact.manifold);
    previous.extend_from_slice(previous_extras(ctx, header));
    manifolds.clear();
    let relative = frame_a.transform.inverse_mul(&frame_b.transform);
    let speculative = ctx.speculative_for(frame_a, frame_b);
    if let Some(cache) = contact.triangulated.as_deref_mut() {
        collide_triangulated(
            shape_a,
            shape_b,
            relative,
            speculative,
            cache,
            mesh,
            manifolds,
        );
    }
    let rotation = Mat3::from_quat(frame_a.transform.q);
    let origin = frame_a.transform.p;
    for manifold in manifolds.iter_mut() {
        manifold.normal = rotation * manifold.normal;
        for point in &mut manifold.points[..manifold.count as usize] {
            point.point = rotation * point.point + origin;
        }
        trim_unreachable(manifold, frame_a, frame_b, ctx.speculative);
    }
    manifolds.retain(|manifold| manifold.count > 0);
    let mut pool = ctx.current.local(worker);
    let start = pool.len() as u32;
    let state = finish_many(
        header,
        contact,
        manifolds,
        previous,
        (frame_a, frame_b),
        ctx.colliders,
        &mut pool,
    );
    let count = pool.len() as u32 - start;
    header.extra = if count == 0 {
        ExtraManifolds::default()
    } else {
        ExtraManifolds {
            worker,
            start,
            count,
            stamp: ctx.stamp,
        }
    };
    if state {
        changed.set(id as usize);
    }
}
