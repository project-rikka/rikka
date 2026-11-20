use rikka_physics_math::Vec3;

use rikka_physics_geometry::*;

fn wave() -> HeightField {
    HeightField::wave(37, 29, Vec3::new(0.5, 1.2, 0.7), 0.05, 0.08, true).expect("wave")
}

#[test]
fn build_validates_grid() {
    assert_eq!(
        HeightField::build(&HeightFieldDef::new(&[0.0; 3], 3, 1)),
        Err(Error::HeightFieldTooSmall {
            rows: 1,
            columns: 3
        })
    );
    assert_eq!(
        HeightField::build(&HeightFieldDef::new(&[0.0; 5], 3, 2)),
        Err(Error::HeightFieldSize {
            expected: 6,
            actual: 5
        })
    );
}

#[test]
fn bytes_round_trip() {
    let field = wave();
    assert_eq!(
        HeightField::from_bytes(field.bytes()).expect("round trip"),
        field
    );
    assert!(HeightField::from_bytes(&field.bytes()[..64]).is_err());
}

#[test]
fn rays_pass_through_holes() {
    let field = HeightField::grid(10, 10, Vec3::ONE, true).expect("grid");
    assert!(field.is_hole(16));
    let cell_center = |cell: u32| Vec3::new((cell % 9) as f32 + 0.5, 1.0, (cell / 9) as f32 + 0.5);
    let down = Vec3::new(0.0, -2.0, 0.0);
    assert!(
        field
            .ray_cast(&RayInput::new(cell_center(16), down))
            .is_none()
    );
    let hit = field
        .ray_cast(&RayInput::new(cell_center(15), down))
        .expect("solid cell");
    assert_eq!(hit.triangle >> 1, 15);
    assert!((hit.fraction - 0.5).abs() < 0.01);
    assert!(hit.normal.y > 0.99);
}
