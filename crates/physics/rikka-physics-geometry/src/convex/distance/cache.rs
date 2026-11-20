use rikka_physics_math::Vec3;

use super::simplex::{MAX_SIMPLEX_VERTICES, Simplex};
use crate::convex::support::{Support, SupportPoint};

const QUANTA: f32 = 65534.0;
pub(crate) const NO_HEADING: u32 = u32::MAX;
const HEADINGS_B: u32 = MAX_SIMPLEX_VERTICES as u32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CachedFeature {
    pub(crate) id: u32,
}

impl CachedFeature {
    #[inline]
    fn from_support(support: SupportPoint, heading: u32) -> (Self, bool) {
        if support.id == SupportPoint::CONTINUOUS {
            (Self { id: heading }, true)
        } else {
            (Self { id: support.id }, false)
        }
    }

    #[inline]
    fn resolve(
        self,
        heading: bool,
        shape: &(impl Support + ?Sized),
    ) -> Option<(SupportPoint, u32)> {
        if heading {
            let support = match decode(self.id) {
                Some(direction) => shape.support(direction),
                None => shape.anchor(),
            };
            return Some((support, self.id));
        }
        shape
            .vertex(self.id)
            .map(|point| (SupportPoint::new(point, self.id), NO_HEADING))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CachedVertex {
    pub(crate) a: SupportPoint,
    pub(crate) b: SupportPoint,
    pub(crate) heading_a: u32,
    pub(crate) heading_b: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimplexCache {
    pub(crate) metric: f32,
    pub(crate) a: [CachedFeature; MAX_SIMPLEX_VERTICES],
    pub(crate) b: [CachedFeature; MAX_SIMPLEX_VERTICES],
    pub(crate) headings: u8,
    pub(crate) count: u8,
}

const _: () = assert!(size_of::<SimplexCache>() == 40);

impl SimplexCache {
    pub const EMPTY: Self = Self {
        metric: 0.0,
        a: [CachedFeature { id: 0 }; MAX_SIMPLEX_VERTICES],
        b: [CachedFeature { id: 0 }; MAX_SIMPLEX_VERTICES],
        headings: 0,
        count: 0,
    };

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::EMPTY;
    }

    pub(crate) fn write(&mut self, simplex: &Simplex) {
        self.metric = simplex.metric();
        self.count = simplex.count as u8;
        let mut headings = 0u32;
        for (index, vertex) in simplex.vertices[..simplex.count].iter().enumerate() {
            let (a, heading_a) = CachedFeature::from_support(vertex.support_a(), vertex.heading_a);
            let (b, heading_b) = CachedFeature::from_support(vertex.support_b(), vertex.heading_b);
            self.a[index] = a;
            self.b[index] = b;
            headings |=
                u32::from(heading_a) << index | u32::from(heading_b) << (HEADINGS_B + index as u32);
        }
        self.headings = headings as u8;
    }

    #[inline]
    pub(crate) fn vertices<A: Support + ?Sized, B: Support + ?Sized>(
        &self,
        a: &A,
        b: &B,
    ) -> Option<[CachedVertex; MAX_SIMPLEX_VERTICES]> {
        let mut out = [CachedVertex::default(); MAX_SIMPLEX_VERTICES];
        let headings = u32::from(self.headings);
        for (index, slot) in out.iter_mut().enumerate().take(self.count as usize) {
            let (support_a, heading_a) = self.a[index].resolve(headings >> index & 1 != 0, a)?;
            let (support_b, heading_b) =
                self.b[index].resolve(headings >> (HEADINGS_B + index as u32) & 1 != 0, b)?;
            *slot = CachedVertex {
                a: support_a,
                b: support_b,
                heading_a,
                heading_b,
            };
        }
        Some(out)
    }
}

#[inline]
pub(crate) fn heading(support: SupportPoint, direction: Vec3) -> u32 {
    if support.id == SupportPoint::CONTINUOUS {
        encode(direction)
    } else {
        NO_HEADING
    }
}

#[inline]
fn sign(value: f32) -> f32 {
    if value < 0.0 { -1.0 } else { 1.0 }
}

#[inline]
fn quantize(value: f32) -> u32 {
    ((value * 0.5 + 0.5).clamp(0.0, 1.0) * QUANTA + 0.5) as u32
}

#[inline]
fn dequantize(value: u32) -> f32 {
    value as f32 / QUANTA * 2.0 - 1.0
}

fn encode(direction: Vec3) -> u32 {
    let norm = direction.abs().element_sum();
    if norm <= 0.0 || !norm.is_finite() {
        return NO_HEADING;
    }
    let n = direction / norm;
    let (u, v) = if n.z >= 0.0 {
        (n.x, n.y)
    } else {
        ((1.0 - n.y.abs()) * sign(n.x), (1.0 - n.x.abs()) * sign(n.y))
    };
    quantize(u) | quantize(v) << 16
}

#[inline]
fn decode(code: u32) -> Option<Vec3> {
    if code == NO_HEADING {
        return None;
    }
    let u = dequantize(code & 0xffff);
    let v = dequantize(code >> 16);
    let z = 1.0 - u.abs() - v.abs();
    let (x, y) = if z < 0.0 {
        ((1.0 - v.abs()) * sign(u), (1.0 - u.abs()) * sign(v))
    } else {
        (u, v)
    };
    Some(Vec3::new(x, y, z))
}
