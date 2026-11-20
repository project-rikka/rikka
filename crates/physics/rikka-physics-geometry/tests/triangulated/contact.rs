use rikka_physics_math::{Quat, Transform, Vec3};

use rikka_physics_geometry::*;

const SPECULATIVE: f32 = 0.02;

fn contacts(
    a: &Shape,
    b: &Shape,
    transform: Transform,
    cache: &mut TriangulatedContact,
) -> Vec<Manifold> {
    let mut scratch = MeshContactScratch::new(Tolerance::default());
    let mut out = Vec::new();
    collide_triangulated(a, b, transform, SPECULATIVE, cache, &mut scratch, &mut out);
    out
}

fn once(a: &Shape, b: &Shape, transform: Transform) -> Vec<Manifold> {
    contacts(a, b, transform, &mut TriangulatedContact::new())
}

fn grid_mesh(identify_edges: bool, materials: bool) -> Shape {
    let cells = 8u32;
    let mut vertices = Vec::new();
    for x in 0..=cells {
        for z in 0..=cells {
            vertices.push(Vec3::new(x as f32 - 4.0, 0.0, z as f32 - 4.0));
        }
    }
    let mut indices = Vec::new();
    let mut material = Vec::new();
    for x in 0..cells {
        for z in 0..cells {
            let i1 = z + (cells + 1) * x;
            let i2 = i1 + 1;
            let i3 = i2 + cells + 1;
            let i4 = i3 - 1;
            indices.extend_from_slice(&[i1, i2, i3, i3, i4, i1]);
            let m = u8::from(materials && x >= 4);
            material.extend_from_slice(&[m, m]);
        }
    }
    let def = MeshDef::new(&vertices, &indices)
        .materials(&material)
        .identify_edges(identify_edges);
    Shape::Mesh(MeshShape::new(
        TriangleMesh::build(&def).expect("grid").mesh,
    ))
}

fn resting(position: Vec3, rotation: Quat) -> Transform {
    Transform::new(position, rotation).inverse()
}

fn horizontal(manifolds: &[Manifold]) -> f32 {
    manifolds
        .iter()
        .map(|manifold| manifold.normal.x.abs().max(manifold.normal.z.abs()))
        .fold(0.0, f32::max)
}

#[test]
fn sliding_box_sees_no_internal_edges() {
    let cube = Shape::Cuboid(Cuboid::new(Vec3::splat(0.5)));
    let smooth = grid_mesh(true, false);
    let raw = grid_mesh(false, false);
    let mut ghosts = 0;
    for step in 0..60 {
        let x = -1.5 + step as f32 * 0.05 + 0.001;
        let transform = resting(Vec3::new(x, 0.49, 0.2), Quat::IDENTITY);
        let manifolds = once(&cube, &smooth, transform);
        assert!(!manifolds.is_empty(), "x {x}");
        assert!(horizontal(&manifolds) < 1e-3, "x {x}: {manifolds:?}");
        for manifold in &manifolds {
            assert!(manifold.normal.y < -0.999);
            assert!(manifold.points().iter().all(|point| point.separation < 0.0));
        }
        if horizontal(&once(&cube, &raw, transform)) > 0.5 {
            ghosts += 1;
        }
    }
    assert!(ghosts > 0);
}

#[test]
fn manifolds_split_by_material() {
    let cube = Shape::Cuboid(Cuboid::new(Vec3::new(1.0, 0.5, 1.0)));
    let mesh = grid_mesh(true, true);
    let manifolds = once(
        &cube,
        &mesh,
        resting(Vec3::new(0.0, 0.49, 0.5), Quat::IDENTITY),
    );
    let mut materials: Vec<u32> = manifolds.iter().map(|manifold| manifold.material).collect();
    materials.sort_unstable();
    assert_eq!(materials, vec![0, 1]);
    assert!(manifolds.iter().all(|manifold| manifold.count >= 2));
}

#[test]
fn height_field_holes_produce_no_contacts() {
    let field = Shape::HeightField(HeightField::grid(10, 10, Vec3::ONE, true).expect("field"));
    let ball = Shape::Sphere(Sphere::new(0.2));
    let center = |cell: u32| Vec3::new((cell % 9) as f32 + 0.5, 0.19, (cell / 9) as f32 + 0.5);
    assert!(once(&ball, &field, resting(center(16), Quat::IDENTITY)).is_empty());
    let solid = once(&ball, &field, resting(center(15), Quat::IDENTITY));
    assert_eq!(solid.len(), 1);
    assert!(solid[0].normal.y < -0.999);
}

#[test]
fn compounds_collide_with_convex_and_compounds() {
    let floor = Compound::build(&[
        CompoundChild::new(
            Cuboid::new(Vec3::new(1.0, 0.5, 1.0)),
            Transform::from_translation(Vec3::new(-1.0, -0.5, 0.0)),
        )
        .material(3),
        CompoundChild::new(
            MeshShape::new(TriangleMesh::grid(4, 4, 0.5, 0).expect("grid")),
            Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        )
        .material(7),
    ])
    .expect("floor");
    let floor = Shape::Compound(floor);
    let ball = Shape::Sphere(Sphere::new(0.25));
    let on_box = once(
        &ball,
        &floor,
        resting(Vec3::new(-1.0, 0.24, 0.0), Quat::IDENTITY),
    );
    assert_eq!(on_box.len(), 1);
    assert_eq!(on_box[0].material, 3);
    assert!(on_box[0].normal.y < -0.999);
    let on_mesh = once(
        &ball,
        &floor,
        resting(Vec3::new(1.1, 0.24, 0.1), Quat::IDENTITY),
    );
    assert_eq!(on_mesh.len(), 1);
    assert_eq!(on_mesh[0].material, 7);
    let pile = Shape::Compound(
        Compound::build(&[
            CompoundChild::new(
                Sphere::new(0.25),
                Transform::from_translation(Vec3::new(-1.0, 0.0, 0.0)),
            ),
            CompoundChild::new(
                Cuboid::new(Vec3::splat(0.25)),
                Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            ),
        ])
        .expect("pile"),
    );
    let pile_to_floor = Transform::from_translation(Vec3::new(0.0, 0.24, 0.0));
    let manifolds = once(&floor, &pile, pile_to_floor);
    assert_eq!(manifolds.len(), 2);
    for manifold in &manifolds {
        assert!(manifold.normal.y > 0.999, "{:?}", manifold.normal);
        for point in manifold.points() {
            assert!(point.point.y.abs() < 0.02);
            assert!(point.separation < 0.0);
        }
    }
    let reversed = once(&pile, &floor, pile_to_floor.inverse());
    assert_eq!(reversed.len(), 2);
    assert!(reversed.iter().all(|manifold| manifold.normal.y < -0.999));
}
