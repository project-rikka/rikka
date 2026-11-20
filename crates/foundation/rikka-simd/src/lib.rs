#[cfg(not(target_arch = "x86_64"))]
compile_error!("rikka-simd targets x86_64 only");

mod compress;
mod convert;
mod denormals;
mod dispatch;
mod element;
mod float;
mod gather;
mod int;
mod level;
mod mask;
mod math;
mod native;
mod pair;
mod prefetch;
mod scalar;
mod sealed;
mod simd;
mod single;
mod swizzle;
mod vector;

pub use compress::Compress;
pub use convert::{Narrow, NarrowUnsigned, Widen};
pub use denormals::FlushDenormals;
pub use element::{Element, FloatElement};
pub use float::Float;
pub use gather::Gather;
pub use int::{Int, SignedInt};
pub use level::{Isa, Level};
pub use mask::Mask;
pub use math::FloatMath;
pub use prefetch::prefetch;
pub use scalar::ScalarFloat;
pub use simd::{Simd, Sse2};
pub use swizzle::Swizzle;
pub use vector::Vector;

pub mod prelude {
    pub use crate::{
        Compress, Element, Float, FloatElement, FloatMath, Gather, Int, Isa, Level, Mask, Narrow,
        NarrowUnsigned, ScalarFloat, SignedInt, Simd, Swizzle, Vector, Widen, dispatch,
    };
}

#[doc(hidden)]
pub mod __private {
    pub use crate::level::Token;
}
