use rikka_simd::{Level, Vector};

use crate::Wide;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Simd(Level);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimdUnavailable {
    pub lanes: usize,
}

impl std::fmt::Display for SimdUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "this processor cannot run {} solver lanes", self.lanes)
    }
}

impl std::error::Error for SimdUnavailable {}

pub trait Kernel {
    type Output;

    fn run<W: Wide>(self) -> Self::Output;
}

impl Simd {
    pub fn detect() -> Self {
        Self(Level::detect())
    }

    pub const fn baseline() -> Self {
        Self(Level::baseline())
    }

    pub const fn new(level: Level) -> Self {
        Self(level)
    }

    pub fn with_lanes(lanes: usize) -> Result<Self, SimdUnavailable> {
        Level::supported()
            .rev()
            .map(Self)
            .find(|simd| simd.lanes() == lanes)
            .ok_or(SimdUnavailable { lanes })
    }

    pub const fn level(self) -> Level {
        self.0
    }

    pub fn lanes(self) -> usize {
        rikka_simd::dispatch!(self.0, simd => lanes_of(simd))
    }

    pub const fn name(self) -> &'static str {
        self.0.name()
    }

    #[inline]
    pub fn run<K: Kernel>(self, kernel: K) -> K::Output {
        rikka_simd::dispatch!(self.0, simd => run_kernel(simd, kernel))
    }
}

#[inline(always)]
fn lanes_of<S: rikka_simd::Simd>(_: S) -> usize {
    if const { <S::F32s as Vector>::LANES >= 8 } {
        <S::F32x8 as Vector>::LANES
    } else {
        <S::F32x4 as Vector>::LANES
    }
}

#[inline(always)]
fn run_kernel<S: rikka_simd::Simd, K: Kernel>(_: S, kernel: K) -> K::Output {
    if const { <S::F32s as Vector>::LANES >= 8 } {
        kernel.run::<S::F32x8>()
    } else {
        kernel.run::<S::F32x4>()
    }
}
