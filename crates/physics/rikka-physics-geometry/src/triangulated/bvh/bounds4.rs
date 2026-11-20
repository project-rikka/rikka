use rikka_physics_math::{Aabb, F32x4, Float, MaskW, Vec3, Vector};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds4 {
    pub(crate) min: [F32x4; 3],
    pub(crate) max: [F32x4; 3],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct QueryBounds4 {
    min: [F32x4; 3],
    max: [F32x4; 3],
}

impl QueryBounds4 {
    #[inline(always)]
    pub(crate) fn new(bounds: &Aabb) -> Self {
        Self {
            min: [
                F32x4::splat(bounds.min.x),
                F32x4::splat(bounds.min.y),
                F32x4::splat(bounds.min.z),
            ],
            max: [
                F32x4::splat(bounds.max.x),
                F32x4::splat(bounds.max.y),
                F32x4::splat(bounds.max.z),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Point4 {
    coords: [F32x4; 3],
}

impl Point4 {
    #[inline(always)]
    pub(crate) fn new(point: Vec3) -> Self {
        Self {
            coords: [
                F32x4::splat(point.x),
                F32x4::splat(point.y),
                F32x4::splat(point.z),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sweep4 {
    origin: [F32x4; 3],
    inverse: [F32x4; 3],
    extent: [F32x4; 3],
}

impl Sweep4 {
    #[inline]
    pub(crate) fn new(origin: Vec3, translation: Vec3, extent: Vec3) -> Self {
        let inverse = |d: f32| {
            let d = if d.abs() < 1.0e-30 {
                1.0e-30f32.copysign(d)
            } else {
                d
            };
            F32x4::splat(1.0 / d)
        };
        Self {
            origin: [
                F32x4::splat(origin.x),
                F32x4::splat(origin.y),
                F32x4::splat(origin.z),
            ],
            inverse: [
                inverse(translation.x),
                inverse(translation.y),
                inverse(translation.z),
            ],
            extent: [
                F32x4::splat(extent.x),
                F32x4::splat(extent.y),
                F32x4::splat(extent.z),
            ],
        }
    }
}

impl Bounds4 {
    #[inline]
    pub(crate) fn from_f32(min: [&[f32; 4]; 3], max: [&[f32; 4]; 3]) -> Self {
        Self {
            min: [
                F32x4::from_array(*min[0]),
                F32x4::from_array(*min[1]),
                F32x4::from_array(*min[2]),
            ],
            max: [
                F32x4::from_array(*max[0]),
                F32x4::from_array(*max[1]),
                F32x4::from_array(*max[2]),
            ],
        }
    }

    #[inline]
    pub(crate) fn lane(&self, slot: usize) -> Aabb {
        Aabb::new(
            Vec3::new(
                self.min[0].lane(slot),
                self.min[1].lane(slot),
                self.min[2].lane(slot),
            ),
            Vec3::new(
                self.max[0].lane(slot),
                self.max[1].lane(slot),
                self.max[2].lane(slot),
            ),
        )
    }

    #[inline(always)]
    pub(crate) fn overlaps_query(&self, query: &QueryBounds4) -> u32 {
        let mut mask = query.max[0].simd_ge(self.min[0]) & self.max[0].simd_ge(query.min[0]);
        mask = mask & query.max[1].simd_ge(self.min[1]) & self.max[1].simd_ge(query.min[1]);
        mask = mask & query.max[2].simd_ge(self.min[2]) & self.max[2].simd_ge(query.min[2]);
        mask.bits()
    }

    #[inline(always)]
    pub(crate) fn overlaps(&self, bounds: &Aabb) -> u32 {
        self.overlaps_query(&QueryBounds4::new(bounds))
    }

    #[inline]
    pub(crate) fn sweep(&self, sweep: &Sweep4, max_fraction: f32) -> (u32, F32x4) {
        let mut lower = F32x4::default();
        let mut upper = F32x4::splat(max_fraction);
        for axis in 0..3 {
            let low = self.min[axis] - sweep.extent[axis] - sweep.origin[axis];
            let high = self.max[axis] + sweep.extent[axis] - sweep.origin[axis];
            let t1 = low * sweep.inverse[axis];
            let t2 = high * sweep.inverse[axis];
            lower = lower.max(t1.min(t2));
            upper = upper.min(t1.max(t2));
        }
        (lower.simd_le(upper).bits(), lower)
    }

    #[inline(always)]
    pub(crate) fn distance_squared(&self, point: &Point4) -> F32x4 {
        let zero = F32x4::default();
        let mut total = zero;
        for axis in 0..3 {
            let p = point.coords[axis];
            let d = (self.min[axis] - p).max(p - self.max[axis]).max(zero);
            total += d * d;
        }
        total
    }
}
