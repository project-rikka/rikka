use rikka_physics_math::{Plane, Transform};

use super::clip::{FeaturePair, MAX_CLIP_POINTS, Owner};
use super::faces::supporting_face;
use super::reduce::reduce;
use super::round::point;
use crate::convex::Convex;
use crate::convex::support::Support;
use crate::{Manifold, ManifoldPoint};

const DEEPEST_ID: u16 = 0x7fff;

pub(crate) fn collide_plane(
    plane: &Plane,
    shape: Convex<'_>,
    transform: Transform,
    speculative: f32,
) -> Manifold {
    let normal = plane.normal;
    let local_normal = transform.rotate_inverse(normal);
    let face = supporting_face(shape, -local_normal).transformed(transform);

    let mut points = [ManifoldPoint::default(); MAX_CLIP_POINTS];
    let mut count = 0;
    let mut deepest_face = f32::MAX;
    for (&position, &id) in face.points().iter().zip(face.ids()) {
        let separation = plane.separation(position);
        deepest_face = deepest_face.min(separation);
        if separation <= speculative && count < MAX_CLIP_POINTS {
            points[count] = point(
                position - 0.5 * separation * normal,
                separation,
                FeaturePair::new(Owner::B, id, Owner::B, id),
            );
            count += 1;
        }
    }

    let core = shape.support(-local_normal).point;
    let deepest = transform.apply(core - shape.radius() * local_normal);
    let separation = plane.separation(deepest);
    if separation < deepest_face - 1.0e-4 * (1.0 + separation.abs())
        && separation <= speculative
        && count < MAX_CLIP_POINTS
    {
        points[count] = point(
            deepest - 0.5 * separation * normal,
            separation,
            FeaturePair::new(Owner::B, DEEPEST_ID, Owner::B, DEEPEST_ID),
        );
        count += 1;
    }

    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    reduce(&mut manifold, &mut points[..count], speculative);
    manifold
}
