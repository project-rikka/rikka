use rikka_physics_math::{Quat, Transform, Vec3};

use crate::support::ray_triangle;
use rikka_physics_geometry::*;

fn wave() -> TriangleMesh {
    TriangleMesh::wave(30, 24, 0.5, 0.7, 0.13, 0.21).expect("wave mesh")
}

#[test]
fn build_rejects_bad_input_and_reports_degenerates() {
    let vertices = [Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::new(2.0, 0.0, 0.0)];
    assert_eq!(
        TriangleMesh::build(&MeshDef::new(&vertices, &[0, 1, 9])),
        Err(Error::IndexOutOfRange {
            index: 9,
            vertices: 4
        })
    );
    assert_eq!(
        TriangleMesh::build(&MeshDef::new(&vertices, &[0, 1])),
        Err(Error::IndexCount { count: 2 })
    );
    assert_eq!(
        TriangleMesh::build(&MeshDef::new(&vertices, &[0, 1, 3])),
        Err(Error::EmptyMesh)
    );
    assert_eq!(
        TriangleMesh::build(&MeshDef::new(&vertices, &[0, 2, 1]).materials(&[1, 2])),
        Err(Error::MaterialCount {
            expected: 1,
            actual: 2
        })
    );
    let build = TriangleMesh::build(&MeshDef::new(&vertices, &[0, 1, 3, 0, 2, 1])).expect("mesh");
    assert_eq!(build.degenerate, vec![0]);
    assert_eq!(build.mesh.triangle_count(), 1);
    assert_eq!(build.mesh.degenerate_count(), 1);
}

#[test]
fn bytes_round_trip_and_reject_corruption() {
    let mesh = wave();
    let copy = TriangleMesh::from_bytes(mesh.bytes()).expect("round trip");
    assert_eq!(copy, mesh);
    let mut bytes = mesh.bytes().to_vec();
    bytes.truncate(bytes.len() / 2);
    assert!(TriangleMesh::from_bytes(&bytes).is_err());
}

#[test]
fn ray_casts_match_brute_force_under_scale() {
    for scale in [
        Vec3::ONE,
        Vec3::new(2.0, 0.5, 1.5),
        Vec3::new(-1.5, 1.0, 1.0),
    ] {
        let shape = MeshShape::scaled(wave(), scale);
        for step in 0..40 {
            let x = -6.0 + 0.3 * step as f32;
            let origin = Vec3::new(x, 3.0, 0.2 * step as f32 - 4.0);
            let translation = Vec3::new(0.7, -6.0, 0.3);
            let expected = (0..shape.mesh.triangle_count())
                .filter_map(|index| {
                    ray_triangle(origin, translation, shape.triangle(index).vertices, 1.0)
                })
                .fold(None, |best: Option<f32>, t| {
                    Some(best.map_or(t, |b| b.min(t)))
                });
            let hit = shape.ray_cast(&RayInput::new(origin, translation));
            match (hit, expected) {
                (Some(hit), Some(expected)) => {
                    assert!((hit.fraction - expected).abs() < 1e-5);
                    assert!(hit.normal.y > 0.0, "{scale} {:?}", hit.normal);
                    let triangle = shape.triangle(hit.triangle);
                    assert!(ray_triangle(origin, translation, triangle.vertices, 1.0).is_some());
                }
                (None, None) => {}
                other => panic!("{scale}: {other:?}"),
            }
        }
    }
}

#[test]
fn only_closed_meshes_have_mass() {
    let cube =
        TriangleMesh::cuboid(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.5, 1.0, 1.5)).expect("box");
    assert!(cube.is_closed());
    for scale in [Vec3::ONE, Vec3::new(-1.0, 1.0, 1.0)] {
        let mass = MeshShape::scaled(cube.clone(), scale).mass(2.0);
        assert!((mass.mass - 2.0 * 6.0).abs() < 1e-3, "{}", mass.mass);
        assert!((mass.center - Vec3::new(scale.x, 2.0, 3.0)).length() < 1e-4);
        let expected = 12.0 * (4.0 + 9.0) / 12.0;
        assert!((mass.inertia.x_axis.x - expected).abs() < 1e-2);
    }
    assert_eq!(MeshShape::new(wave()).mass(1.0).mass, 0.0);
    let torus = TriangleMesh::torus(16, 24, 2.0, 0.5).expect("torus");
    assert!(torus.is_closed());
    let volume = MeshShape::new(torus).mass(1.0).mass;
    let exact = 2.0 * std::f32::consts::PI * std::f32::consts::PI * 2.0 * 0.25;
    assert!((volume - exact).abs() < 0.05 * exact);
}

#[test]
fn overlap_and_shape_cast_find_surface() {
    let shape = MeshShape::new(TriangleMesh::grid(10, 10, 1.0, 0).expect("grid"));
    let points = [Vec3::new(-0.2, 0.0, 0.0), Vec3::new(0.2, 0.0, 0.0)];
    let proxy = ShapeProxy::new(&points, 0.25);
    assert!(shape.overlaps(
        Transform::from_translation(Vec3::new(0.0, -0.2, 0.0)),
        &proxy
    ));
    assert!(!shape.overlaps(
        Transform::from_translation(Vec3::new(0.0, -0.4, 0.0)),
        &proxy
    ));
    let rotated = Transform::new(Vec3::new(0.0, -0.3, 0.0), Quat::from_rotation_x(0.3));
    assert!(!shape.overlaps(rotated, &proxy));
    let lifted = [points[0] + Vec3::Y, points[1] + Vec3::Y];
    let cast = ShapeCastInput {
        proxy: ShapeProxy::new(&lifted, 0.25),
        translation: Vec3::new(0.0, -2.0, 0.0),
        max_fraction: 1.0,
        can_encroach: false,
    };
    let hit = shape.shape_cast(&cast).expect("hits the grid");
    assert!((hit.fraction - 0.375).abs() < 0.01, "{}", hit.fraction);
    assert!(hit.normal.y > 0.99);
}
