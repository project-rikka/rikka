use rikka_physics_core::Shared;
pub(crate) use rikka_physics_math::MAX_LANES;
use rikka_physics_math::{QuatW, Vec3W, Wide};

use super::state::{BodyState, DYNAMIC};
use super::sym::SymW;
use crate::set::BodyMotion;

pub(crate) type LaneIndices = [u32; MAX_LANES];

#[derive(Clone, Copy, Debug)]
pub(crate) struct BodyW<W: Wide> {
    pub v: Vec3W<W>,
    pub w: Vec3W<W>,
    pub dp: Vec3W<W>,
    pub dq: QuatW<W>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VelocityW<W: Wide> {
    pub v: Vec3W<W>,
    pub w: Vec3W<W>,
}

#[inline(always)]
pub(crate) unsafe fn transpose<W: Wide>(rows: &[*const f32; MAX_LANES], offset: usize) -> [W; 4] {
    unsafe { W::load_rows(&rows.map(|row| row.wrapping_add(offset))) }
}

#[inline(always)]
pub(crate) unsafe fn untranspose<W: Wide>(
    columns: [W; 4],
    rows: &[*mut f32; MAX_LANES],
    offset: usize,
) {
    unsafe { W::store_rows(columns, &rows.map(|row| row.wrapping_add(offset))) }
}

#[inline(always)]
fn to_rows<W: Wide>(columns: [W; 4]) -> [[f32; 4]; MAX_LANES] {
    let mut rows = [[0.0f32; 4]; MAX_LANES];
    W::store_transposed(columns, &mut rows.each_mut()[..W::LANES]);
    rows
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LaneKinds {
    pub dynamic: u32,
    pub kinematic: u32,
}

impl LaneKinds {
    pub(crate) const EMPTY: Self = Self {
        dynamic: 0,
        kinematic: 0,
    };
}

#[inline(always)]
pub(crate) fn load_masses<W: Wide>(
    motion: Shared<'_, BodyMotion>,
    split: &[f32],
    indices: &LaneIndices,
) -> (W, SymW<W>) {
    const {
        assert!(
            std::mem::offset_of!(BodyMotion, inv_inertia)
                == std::mem::offset_of!(BodyMotion, inv_mass) + size_of::<f32>()
        )
    };
    static STATIC_BODY: [f32; 7] = [0.0; 7];
    let rows: [*const f32; MAX_LANES] = std::array::from_fn(|lane| {
        if lane < W::LANES && indices[lane] != 0 {
            let body = unsafe { motion.at(indices[lane] as usize - 1) };
            std::ptr::from_ref(&body.inv_mass)
        } else {
            STATIC_BODY.as_ptr()
        }
    });
    let [inv_mass, xx, xy, xz] = unsafe { transpose::<W>(&rows, 0) };
    let [_, yy, yz, zz] = unsafe { transpose::<W>(&rows, 3) };
    let inertia = SymW {
        xx,
        xy,
        xz,
        yy,
        yz,
        zz,
    };
    if split.is_empty() {
        return (inv_mass, inertia);
    }
    let scale = W::from_fn(|lane| match indices[lane] {
        0 => 0.0,
        index => split[index as usize - 1],
    });
    (
        inv_mass * scale,
        SymW {
            xx: xx * scale,
            xy: xy * scale,
            xz: xz * scale,
            yy: yy * scale,
            yz: yz * scale,
            zz: zz * scale,
        },
    )
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct States<'a> {
    states: Shared<'a, BodyState>,
    kinematic_steps: u32,
    kinematic_h: f32,
}

impl<'a> States<'a> {
    #[inline]
    pub(crate) fn new(states: Shared<'a, BodyState>) -> Self {
        Self {
            states,
            kinematic_steps: 0,
            kinematic_h: 0.0,
        }
    }

    #[inline]
    pub(crate) fn with_kinematic_steps(self, steps: u32, h: f32) -> Self {
        Self {
            kinematic_steps: steps,
            kinematic_h: h,
            ..self
        }
    }

    #[inline(always)]
    pub(crate) fn kinds<W: Wide>(&self, indices: &LaneIndices) -> LaneKinds {
        let mut kinds = LaneKinds::default();
        for (lane, &index) in indices.iter().enumerate().take(W::LANES) {
            if index == 0 {
                continue;
            }
            if unsafe { (*self.row(index)).flags } & DYNAMIC != 0 {
                kinds.dynamic |= 1 << lane;
            } else {
                kinds.kinematic |= 1 << lane;
            }
        }
        kinds
    }

    #[inline(always)]
    fn advance_kinematic<W: Wide>(&self, kinematic: u32, body: &mut BodyW<W>) {
        if kinematic == 0 || self.kinematic_steps == 0 {
            return;
        }
        let kinematic = W::from_fn(|lane| (kinematic >> lane & 1) as f32).simd_gt(W::default());
        let h = W::splat(self.kinematic_h);
        let mut moved = Vec3W::zero();
        let mut turned = QuatW::identity();
        for _ in 0..self.kinematic_steps {
            moved = body.v.mul_add(h, moved);
            turned = turned.integrate(body.w, h);
        }
        body.dp = Vec3W::select(kinematic, moved, body.dp);
        body.dq = QuatW::select(kinematic, turned, body.dq);
    }

    #[inline(always)]
    fn row(&self, index: u32) -> *const BodyState {
        if index == 0 {
            &BodyState::IDENTITY
        } else {
            debug_assert!((index as usize) <= self.states.len());
            unsafe { self.states.as_ptr().add(index as usize - 1) }
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn state(&self, index: u32) -> &'a mut BodyState {
        unsafe { self.states.get(index as usize - 1) }
    }

    #[inline(always)]
    fn rows<W: Wide>(&self, indices: &LaneIndices) -> [*const f32; MAX_LANES] {
        std::array::from_fn(|lane| {
            if lane < W::LANES {
                self.row(indices[lane]).cast::<f32>()
            } else {
                (&BodyState::IDENTITY as *const BodyState).cast::<f32>()
            }
        })
    }

    #[inline(always)]
    fn load_bodies<W: Wide>(&self, indices: &LaneIndices) -> BodyW<W> {
        let rows = self.rows::<W>(indices);
        unsafe {
            let [vx, vy, vz, _] = transpose::<W>(&rows, 0);
            let [wx, wy, wz, _] = transpose::<W>(&rows, 4);
            let [px, py, pz, _] = transpose::<W>(&rows, 8);
            let [qx, qy, qz, qs] = transpose::<W>(&rows, 12);
            BodyW {
                v: Vec3W::new(vx, vy, vz),
                w: Vec3W::new(wx, wy, wz),
                dp: Vec3W::new(px, py, pz),
                dq: QuatW::new(Vec3W::new(qx, qy, qz), qs),
            }
        }
    }

    #[inline(always)]
    pub(crate) fn gather<W: Wide>(&self, indices: &LaneIndices) -> BodyW<W> {
        let mut body = self.load_bodies(indices);
        if self.kinematic_steps != 0 {
            self.advance_kinematic(self.kinds::<W>(indices).kinematic, &mut body);
        }
        body
    }

    #[inline(always)]
    pub(crate) fn gather_kinds<W: Wide>(
        &self,
        indices: &LaneIndices,
        kinds: LaneKinds,
    ) -> BodyW<W> {
        let mut body = self.load_bodies(indices);
        self.advance_kinematic(kinds.kinematic, &mut body);
        body
    }

    #[inline(always)]
    pub(crate) fn gather_velocity<W: Wide>(&self, indices: &LaneIndices) -> VelocityW<W> {
        let rows = self.rows::<W>(indices);
        unsafe {
            let [vx, vy, vz, _] = transpose::<W>(&rows, 0);
            let [wx, wy, wz, _] = transpose::<W>(&rows, 4);
            VelocityW {
                v: Vec3W::new(vx, vy, vz),
                w: Vec3W::new(wx, wy, wz),
            }
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn scatter_kinds<W: Wide>(
        &self,
        indices: &LaneIndices,
        kinds: LaneKinds,
        v: &Vec3W<W>,
        w: &Vec3W<W>,
    ) {
        let mut sinks = [[0.0f32; 8]; MAX_LANES];
        let sinks = sinks.as_mut_ptr();
        let base: [*mut f32; MAX_LANES] = std::array::from_fn(|lane| {
            if kinds.dynamic >> lane & 1 != 0 {
                unsafe { std::ptr::from_mut(self.states.get(indices[lane] as usize - 1)).cast() }
            } else {
                sinks.wrapping_add(lane).cast()
            }
        });
        unsafe {
            untranspose([v.x, v.y, v.z, W::default()], &base, 0);
            untranspose([w.x, w.y, w.z, W::default()], &base, 4);
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn scatter_all<W: Wide>(&self, indices: &LaneIndices, body: &BodyW<W>) {
        let linear = to_rows([body.v.x, body.v.y, body.v.z, W::default()]);
        let angular = to_rows([body.w.x, body.w.y, body.w.z, W::default()]);
        let position = to_rows([body.dp.x, body.dp.y, body.dp.z, W::default()]);
        let rotation = to_rows([body.dq.v.x, body.dq.v.y, body.dq.v.z, body.dq.s]);
        for lane in 0..W::LANES {
            let index = indices[lane];
            if index == 0 {
                continue;
            }
            let state = unsafe { self.states.get(index as usize - 1) };
            let [x, y, z, _] = linear[lane];
            state.v = rikka_physics_math::Vec3::new(x, y, z);
            let [x, y, z, _] = angular[lane];
            state.w = rikka_physics_math::Vec3::new(x, y, z);
            let [x, y, z, _] = position[lane];
            state.dp = rikka_physics_math::Vec3::new(x, y, z);
            let [x, y, z, s] = rotation[lane];
            state.dq = rikka_physics_math::Quat::from_xyzw(x, y, z, s);
        }
    }
}
