use rikka_physics_math::{Mat3, Vec3};

use crate::MassData;

pub(crate) fn closed_surface_mass(
    reference: Vec3,
    density: f32,
    triangles: impl IntoIterator<Item = [Vec3; 3]>,
) -> MassData {
    let canonical = Mat3::from_cols(
        Vec3::new(2.0, 1.0, 1.0),
        Vec3::new(1.0, 2.0, 1.0),
        Vec3::new(1.0, 1.0, 2.0),
    ) * (1.0 / 120.0);
    let mut volume = 0.0f32;
    let mut first_moment = Vec3::ZERO;
    let mut covariance = Mat3::ZERO;
    for [a, b, c] in triangles {
        let (a, b, c) = (a - reference, b - reference, c - reference);
        let columns = Mat3::from_cols(a, b, c);
        let determinant = columns.determinant();
        volume += determinant / 6.0;
        first_moment += determinant / 24.0 * (a + b + c);
        covariance += columns * canonical * columns.transpose() * determinant;
    }
    if volume <= f32::EPSILON {
        return MassData::ZERO;
    }
    let center = first_moment / volume;
    let offset = Mat3::from_cols(center * center.x, center * center.y, center * center.z);
    let central = covariance - offset * volume;
    let trace = central.x_axis.x + central.y_axis.y + central.z_axis.z;
    let inertia = (Mat3::from_diagonal(Vec3::splat(trace)) - central) * density;
    MassData {
        mass: density * volume,
        center: center + reference,
        inertia,
    }
}
