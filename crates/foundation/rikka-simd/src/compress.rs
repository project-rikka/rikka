use crate::Vector;

pub trait Compress: Vector {
    fn compress(self, mask: Self::Mask) -> Self;
}
