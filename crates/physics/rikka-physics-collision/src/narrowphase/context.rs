use rikka_physics_core::{BitSet, Shared};
use rikka_physics_geometry::{Manifold, Shape};

use super::finish::{finish, trim_unreachable};
use crate::contact::{ContactGeometry, ContactHeader};
use crate::{BodyFrame, Collider};

#[derive(Clone, Copy, Debug)]
pub(crate) struct KernelContext<'a> {
    pub(crate) previous: &'a [super::pools::PoolView],
    pub(crate) current: &'a rikka_physics_core::PerWorker<Vec<crate::ContactManifold>>,
    pub(crate) scratch: &'a rikka_physics_core::PerWorker<super::triangulated::TriangulatedScratch>,
    pub(crate) stamp: u32,
    pub headers: Shared<'a, ContactHeader>,
    pub contacts: Shared<'a, ContactGeometry>,
    pub colliders: &'a [Collider],
    pub frames: &'a [BodyFrame],
    pub speculative: f32,
}

impl<'a> KernelContext<'a> {
    #[inline(always)]
    pub(crate) fn header(&self, id: u32) -> &'a mut ContactHeader {
        unsafe { self.headers.get(id as usize) }
    }

    #[inline(always)]
    pub(crate) fn contact(&self, id: u32) -> &'a mut ContactGeometry {
        unsafe { self.contacts.get(id as usize) }
    }

    #[inline(always)]
    pub(crate) fn shapes(&self, header: &ContactHeader) -> (&'a Shape, &'a Shape) {
        (
            &self.colliders[header.collider_a as usize].shape,
            &self.colliders[header.collider_b as usize].shape,
        )
    }

    #[inline(always)]
    pub(crate) fn frames(&self, header: &ContactHeader) -> (&'a BodyFrame, &'a BodyFrame) {
        (
            &self.frames[header.frame_a as usize],
            &self.frames[header.frame_b as usize],
        )
    }

    #[inline(always)]
    pub(crate) fn speculative_for(&self, a: &BodyFrame, b: &BodyFrame) -> f32 {
        let approach = (a.motion - b.motion).length() + a.sweep + b.sweep;
        self.speculative.max(approach)
    }

    #[inline(always)]
    pub(crate) fn complete(&self, id: u32, manifold: &Manifold, changed: &mut BitSet) {
        let header = self.header(id);
        let (a, b) = self.frames(header);
        let mut manifold = *manifold;
        trim_unreachable(&mut manifold, a, b, self.speculative);
        if finish(header, self.contact(id), &manifold, a, b, self.colliders) {
            changed.set(id as usize);
        }
    }
}
