use crate::Vector;

pub trait Swizzle: Vector {
    fn swizzle_dyn(self, indices: Self) -> Self;
}
