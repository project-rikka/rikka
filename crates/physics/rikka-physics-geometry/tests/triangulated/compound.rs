use rikka_physics_math::{Quat, Transform, Vec3};

use rikka_physics_geometry::*;

fn children() -> Vec<CompoundChild> {
    let hull = ConvexHull::new(&[
        Vec3::ZERO,
        Vec3::X,
        Vec3::Y,
        Vec3::Z,
        Vec3::new(0.7, 0.7, 0.7),
    ])
    .expect("hull");
    let ramp = TriangleMesh::cuboid(Vec3::ZERO, Vec3::new(1.0, 0.25, 1.0)).expect("mesh");
    vec![
        CompoundChild::new(
            Sphere::with_center(Vec3::new(0.0, 0.5, 0.0), 0.5),
            Transform::IDENTITY,
        )
        .material(1),
        CompoundChild::new(
            Capsule::tapered(Vec3::ZERO, 0.2, Vec3::new(0.0, 1.0, 0.0), 0.3),
            Transform::from_translation(Vec3::new(3.0, 0.0, 0.0)),
        )
        .material(2),
        CompoundChild::new(
            Cuboid::new(Vec3::new(0.5, 0.25, 0.75)),
            Transform::new(Vec3::new(-3.0, 0.0, 1.0), Quat::from_rotation_y(0.4)),
        )
        .material(3),
        CompoundChild::new(
            Cylinder::new(0.5, 0.3),
            Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
        )
        .material(4),
        CompoundChild::new(
            hull.clone(),
            Transform::from_translation(Vec3::new(6.0, 0.0, 0.0)),
        )
        .material(5),
        CompoundChild::new(hull, Transform::from_translation(Vec3::new(8.0, 0.0, 0.0))).material(5),
        CompoundChild::new(
            MeshShape::scaled(ramp, Vec3::new(2.0, 1.0, 1.0)),
            Transform::from_translation(Vec3::new(0.0, -2.0, 0.0)),
        )
        .material(10),
    ]
}

#[test]
fn bytes_round_trip_shares_embedded_blobs() {
    let source = children();
    let compound = Compound::build(&source).expect("compound");
    let copy = Compound::from_bytes(compound.bytes()).expect("round trip");
    assert_eq!(copy, compound);
    assert_eq!(copy.child_count(), source.len() as u32);
    for (index, child) in source.iter().enumerate() {
        let decoded = copy.child(index as u32);
        assert_eq!(decoded.transform, child.transform);
        assert_eq!(decoded.material, child.material);
        assert_eq!(decoded.shape.kind(), child.shape.kind());
        assert_eq!(
            decoded.shape.aabb(decoded.transform),
            child.shape.aabb(child.transform)
        );
    }
    let (Shape::Hull(first), Shape::Hull(second)) = (copy.child(4).shape, copy.child(5).shape)
    else {
        panic!("hull children");
    };
    assert!(first.blob().shares_storage(second.blob()));
    let hull_bytes = first.blob().bytes().len();
    assert!(compound.bytes().len() < 2 * hull_bytes + 4096);
    assert!(Compound::from_bytes(&compound.bytes()[..40]).is_err());
}

#[test]
fn ray_cast_hits_nearest_child() {
    let source = children();
    let compound = Compound::build(&source).expect("compound");
    for step in 0..40 {
        let origin = Vec3::new(-4.0 + 0.3 * step as f32, 5.0, 0.1 * step as f32 - 0.5);
        let ray = RayInput::new(origin, Vec3::new(0.0, -10.0, 0.0));
        let expected = source
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                let local = RayInput {
                    origin: child.transform.apply_inverse(ray.origin),
                    translation: child.transform.rotate_inverse(ray.translation),
                    max_fraction: 1.0,
                };
                child
                    .shape
                    .ray_cast(&local)
                    .map(|hit| (index as u32, hit.fraction))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let hit = compound.ray_cast(&ray);
        match (hit, expected) {
            (Some(hit), Some((child, fraction))) => {
                assert!((hit.fraction - fraction).abs() < 1e-5);
                assert_eq!(hit.child, child);
                assert!(hit.material >= source[child as usize].material);
            }
            (None, None) => {}
            other => panic!("step {step}: {other:?}"),
        }
    }
}
