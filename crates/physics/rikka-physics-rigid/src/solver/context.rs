use rikka_physics_collision::BodyFrame;
use rikka_physics_core::{BitSet, PerWorker, Shared};

use super::contact::{ContactContext, IterationContext};
use super::integrate::IntegrateContext;
use super::plan::Plan;
use super::softness::Softness;
use super::wide_buffer::Plain;
use crate::BodyMoveEvent;
use crate::body::BodyRecord;
use crate::joint::{JointKind, JointPrepareContext, JointRecord, JointSolveContext};
use crate::set::BodyPose;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawBuffer {
    ptr: *mut u8,
    bytes: usize,
}

unsafe impl Send for RawBuffer {}
unsafe impl Sync for RawBuffer {}

impl RawBuffer {
    pub(crate) fn new(ptr: *mut u8, bytes: usize) -> Self {
        Self { ptr, bytes }
    }

    #[inline(always)]
    pub(crate) unsafe fn at<T: Plain>(self, index: usize, stride: usize) -> *mut T {
        debug_assert!((index + 1) * stride <= self.bytes);
        unsafe { self.ptr.add(index * stride).cast::<T>() }
    }
}

#[derive(Debug, Default)]
pub(crate) struct FinalizeOutput {
    pub enlarged: Vec<u32>,
    pub awake_islands: BitSet,
    pub fast: Vec<u32>,
    pub bullets: Vec<u32>,
    pub moving_kinematic: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FinalizeContext<'a> {
    pub bodies: Shared<'a, BodyRecord>,
    pub body_ids: &'a [u32],
    pub shapes: &'a [crate::shape_record::ShapeRecord],
    pub colliders: &'a [rikka_physics_collision::Collider],
    pub fat: Shared<'a, rikka_physics_math::Aabb>,
    pub moves: Shared<'a, BodyMoveEvent>,
    pub generations: &'a [u32],
    pub output: &'a PerWorker<FinalizeOutput>,
    pub dt: f32,
    pub inv_dt: f32,
    pub sleeping: bool,
    pub time_to_sleep: f32,
    pub slop: f32,
    pub speculative_cap: f32,
    pub tolerance: rikka_physics_geometry::Tolerance,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StepContext<'a> {
    pub plan: &'a Plan,
    pub simd: rikka_physics_math::Simd,
    pub integrate: IntegrateContext<'a>,
    pub poses: Shared<'a, BodyPose>,
    pub frames: Shared<'a, BodyFrame>,
    pub contact: ContactContext<'a>,
    pub iteration: IterationContext,
    pub joints: Shared<'a, JointRecord>,
    pub joint_prepare: JointPrepareContext,
    pub joint_solve: JointSolveContext,
    pub joint_soft: Softness,
    pub contact_blocks: RawBuffer,
    pub contact_lanes: RawBuffer,
    pub joint_blocks: [RawBuffer; JointKind::COUNT],
    pub joint_lanes: RawBuffer,
    pub overflow_deltas: Shared<'a, [rikka_physics_math::Vec3; 4]>,
    pub substeps: u32,
    pub finalize: FinalizeContext<'a>,
}
