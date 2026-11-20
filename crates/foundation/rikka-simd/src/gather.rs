use crate::{Int, Vector};

pub trait Gather: Vector {
    type Index: Int<Simd = Self::Simd, Mask = Self::Mask>;

    fn gather(slice: &[Self::Elem], indices: Self::Index) -> Self;
}

#[cold]
#[inline(never)]
pub(crate) fn index_out_of_range(len: usize) -> ! {
    panic!("gather index out of bounds for slice of length {len}")
}
