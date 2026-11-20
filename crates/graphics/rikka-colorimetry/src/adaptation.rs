use crate::chromaticity::Chromaticity;
use crate::matrix::Matrix3;

const BRADFORD: Matrix3 = Matrix3([
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
]);

pub fn bradford(from: Chromaticity, to: Chromaticity) -> Matrix3 {
    if from == to {
        return Matrix3::IDENTITY;
    }
    let source = BRADFORD.apply(from.to_xyz());
    let destination = BRADFORD.apply(to.to_xyz());
    let scale = Matrix3::diagonal(std::array::from_fn(|channel| {
        destination[channel] / source[channel]
    }));
    let inverse = BRADFORD
        .inverse()
        .expect("the Bradford matrix is invertible");
    inverse * scale * BRADFORD
}
