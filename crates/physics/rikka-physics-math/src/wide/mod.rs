mod mat3;
mod quat;
mod vec3;

use rikka_simd::{Float, FloatMath, Mask, Simd, Sse2};

pub use mat3::Mat3W;
pub use quat::QuatW;
pub use vec3::Vec3W;

pub type F32x1 = <Sse2 as Simd>::F32x1;
pub type F32x4 = <Sse2 as Simd>::F32x4;
pub type M32x1 = <Sse2 as Simd>::M32x1;
pub type M32x4 = <Sse2 as Simd>::M32x4;

pub const MAX_LANES: usize = 8;

pub trait MaskW: Mask {
    #[inline(always)]
    fn bits(self) -> u32 {
        self.to_bitmask() as u32
    }
}

impl<M: Mask> MaskW for M {}

pub trait Wide: Float<Elem = f32> + FloatMath {
    #[inline(always)]
    fn one() -> Self {
        Self::splat(1.0)
    }

    #[inline(always)]
    fn masked(self, mask: Self::Mask) -> Self {
        Self::select(mask, self, Self::default())
    }
    #[inline(always)]
    unsafe fn load_rows(rows: &[*const f32; MAX_LANES]) -> [Self; 4] {
        let first = unsafe { &*rows[0].cast::<[f32; 4]>() };
        let sources: [&[f32; 4]; MAX_LANES] = std::array::from_fn(|lane| {
            if lane < Self::LANES {
                unsafe { &*rows[lane].cast::<[f32; 4]>() }
            } else {
                first
            }
        });
        Self::load_transposed(&sources[..Self::LANES])
    }

    #[inline(always)]
    unsafe fn store_rows(columns: [Self; 4], rows: &[*mut f32; MAX_LANES]) {
        let mut spare = [[0.0f32; 4]; MAX_LANES];
        let mut targets = spare.each_mut();
        for (target, &row) in targets.iter_mut().zip(rows).take(Self::LANES) {
            *target = unsafe { &mut *row.cast::<[f32; 4]>() };
        }
        Self::store_transposed(columns, &mut targets[..Self::LANES]);
    }
}

impl<V: Float<Elem = f32>> Wide for V {}
