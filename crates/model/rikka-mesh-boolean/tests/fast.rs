use std::f32::consts::FRAC_PI_2;

use glam::{EulerRot, Mat4, Vec3};
use rikka_mesh_boolean::{Boolean, Operand, Operation, Solver};
use rikka_mesh_edit::Primitive;
use rikka_polymesh::Polymesh;

use crate::support::{
    Expected, assert_stats, cube, expect, listed_cube, moved, placement, primitive, stats, torus,
    uv_sphere,
};

const EMPTY: Expected = Expected {
    verts: 0,
    edges: 0,
    faces: 0,
    volume: 0.0,
    area: 0.0,
    manifold: true,
    quads: 0,
};

fn check_case(label: &str, a: &Polymesh, b: &Polymesh, transform: Mat4, expected: [Expected; 3]) {
    let operations = [
        Operation::Intersect,
        Operation::Union,
        Operation::Difference,
    ];
    for (operation, expected) in operations.into_iter().zip(expected) {
        let mesh = Boolean::new(operation)
            .operand(a)
            .operand(Operand::new(b).transform(transform))
            .solver(Solver::Fast)
            .compute()
            .unwrap()
            .mesh;
        assert_stats(&mesh, expected, &format!("fast {label} {operation:?}"));
    }
}

#[test]
fn offset_cubes_match_blender() {
    check_case(
        "offset",
        &cube(),
        &cube(),
        moved([1.0, 0.5, 0.25]),
        [
            expect(8, 12, 6, 2.625, 11.75, true, 6),
            expect(20, 30, 12, 13.375, 36.25, true, 6),
            expect(14, 21, 9, 5.375, 24.0, true, 6),
        ],
    );
}

#[test]
fn touching_cubes_match_blender() {
    for (label, location) in [
        ("shared edge", [2.0, 2.0, 0.0]),
        ("shared vertex", [2.0, 2.0, 2.0]),
    ] {
        check_case(
            label,
            &cube(),
            &cube(),
            moved(location),
            [
                EMPTY,
                expect(16, 24, 12, 16.0, 48.0, true, 12),
                expect(8, 12, 6, 8.0, 24.0, true, 6),
            ],
        );
    }
}

#[test]
fn contained_cube_matches_blender() {
    check_case(
        "contained",
        &cube(),
        &cube(),
        placement([0.1, 0.2, 0.3], [0.0; 3], 0.5),
        [
            expect(8, 12, 6, 1.0, 6.0, true, 6),
            expect(8, 12, 6, 8.0, 24.0, true, 6),
            expect(16, 24, 12, 7.0, 30.0, true, 12),
        ],
    );
}

#[test]
fn cube_sphere_matches_blender() {
    check_case(
        "sphere",
        &cube(),
        &uv_sphere(),
        moved([0.6, 0.4, 0.3]),
        [
            expect(274, 529, 257, 2.651_695_2, 10.284_385_0, true, 200),
            expect(348, 673, 327, 9.470_243_5, 26.181_302_9, true, 267),
            expect(282, 541, 263, 5.348_304_8, 24.358_019_7, true, 205),
        ],
    );
}

#[test]
fn torus_torus_matches_blender() {
    check_case(
        "torus",
        &torus(),
        &torus(),
        placement([0.3, 0.1, 0.05], [FRAC_PI_2, 0.0, 0.0], 1.0),
        [
            expect(145, 252, 111, 0.046_156_36, 1.098_248_9, true, 53),
            expect(1159, 2280, 1117, 2.303_315_1, 18.381_447_9, true, 1050),
            expect(653, 1268, 615, 1.128_579_4, 9.741_874_8, true, 552),
        ],
    );
}

#[test]
fn many_operand_boolean_applies_operands_in_turn() {
    let b = cube();
    let locations = [[1.5, 0.3, 0.25], [-1.4, 0.6, -0.3], [0.15, -1.6, 0.45]];
    let cases = [
        (
            Operation::Union,
            expect(44, 66, 24, 18.438_500_7, 52.340_000_3, true, 10),
        ),
        (
            Operation::Difference,
            expect(28, 42, 16, 6.150_499_9, 24.539_999_8, true, 10),
        ),
    ];
    for solver in [Solver::Fast, Solver::Exact] {
        for (operation, expected) in cases {
            let operands = locations
                .iter()
                .map(|&location| Operand::new(&b).transform(placement(location, [0.0; 3], 0.8)));
            let mesh = Boolean::new(operation)
                .operand(&cube())
                .operands(operands)
                .solver(solver)
                .compute()
                .unwrap()
                .mesh;
            assert_stats(&mesh, expected, &format!("many {solver:?} {operation:?}"));
        }
    }
}

fn both_solvers(
    a: &Polymesh,
    b: &Polymesh,
    transform: Mat4,
    operation: Operation,
) -> [(Solver, Polymesh); 2] {
    [Solver::Exact, Solver::Fast].map(|solver| {
        let mesh = Boolean::new(operation)
            .operand(a)
            .operand(Operand::new(b).transform(transform))
            .solver(solver)
            .compute()
            .unwrap()
            .mesh;
        (solver, mesh)
    })
}

#[test]
fn corner_cuts_match_blender() {
    let cases = [
        (Operation::Intersect, expect(8, 12, 6, 0.165, 1.81, true, 6)),
        (Operation::Union, expect(20, 30, 12, 8.555, 27.03, true, 6)),
        (
            Operation::Difference,
            expect(14, 21, 9, 7.835, 24.0, true, 6),
        ),
    ];
    for (layout, target) in [("listed", listed_cube()), ("primitive", cube())] {
        for corner in 0..8 {
            let signs = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let transform =
                Mat4::from_translation(signs * 0.9) * Mat4::from_scale(Vec3::new(0.4, 0.45, 0.5));
            for (operation, expected) in cases {
                for (solver, mesh) in both_solvers(&target, &target, transform, operation) {
                    assert_stats(
                        &mesh,
                        expected,
                        &format!("{layout} corner {signs} {solver:?} {operation:?}"),
                    );
                }
            }
        }
    }
}

#[test]
fn rotated_operand_matches_blender() {
    let target = Mat4::from_cols_array(&[
        1.2, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.2, 0.0, 0.1, 1.0,
    ]);
    let other = Mat4::from_cols_array(&[
        0.4,
        0.0,
        0.0,
        0.0,
        0.0,
        0.382_134_62,
        0.118_208_09,
        0.0,
        0.0,
        -0.265_968_17,
        0.859_802_84,
        0.0,
        -1.0,
        -0.6,
        -0.5,
        1.0,
    ]);
    let cube = listed_cube();
    let cases = [
        (
            Operation::Intersect,
            expect(10, 15, 7, 0.315_314_9, 3.243_339_9, true, 5),
        ),
        (
            Operation::Union,
            expect(24, 36, 14, 8.644_685_3, 27.103_327_3, true, 6),
        ),
        (
            Operation::Difference,
            expect(18, 27, 11, 7.684_685_1, 24.207_318_5, true, 6),
        ),
    ];
    for (operation, expected) in cases {
        for (solver, mesh) in both_solvers(&cube, &cube, target.inverse() * other, operation) {
            assert_stats(
                &mesh,
                expected,
                &format!("rotated {solver:?} {operation:?}"),
            );
        }
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

#[test]
fn fast_agrees_with_exact_on_generic_convex_operands() {
    let mut random = Lcg(0x5eed_b001);
    let target = listed_cube();
    let shapes = [
        listed_cube(),
        primitive(
            &Primitive::IcoSphere {
                subdivisions: 2,
                radius: 1.0,
            },
            Mat4::IDENTITY,
        ),
    ];
    for trial in 0..24 {
        let shape = &shapes[trial % shapes.len()];
        let rotation = [0; 3].map(|_| random.range(0.0, std::f32::consts::TAU));
        let scale = Vec3::new(
            random.range(0.3, 0.9),
            random.range(0.3, 0.9),
            random.range(0.3, 0.9),
        );
        let location = Vec3::new(
            random.range(-1.3, 1.3),
            random.range(-1.3, 1.3),
            random.range(-1.3, 1.3),
        );
        let transform = Mat4::from_translation(location)
            * Mat4::from_euler(EulerRot::ZYX, rotation[2], rotation[1], rotation[0])
            * Mat4::from_scale(scale);
        for operation in [
            Operation::Intersect,
            Operation::Union,
            Operation::Difference,
        ] {
            let [(_, exact), (_, fast)] = both_solvers(&target, shape, transform, operation);
            exact.validate().unwrap();
            fast.validate().unwrap();
            let (exact, fast) = (stats(&exact), stats(&fast));
            let label = format!("trial {trial} {operation:?}");
            assert!(
                exact.manifold && fast.manifold,
                "{label}: exact {exact:?} fast {fast:?}"
            );
            assert!(
                (exact.volume - fast.volume).abs() <= 1.0e-4 * exact.volume.abs().max(1.0),
                "{label}: exact {exact:?} fast {fast:?}"
            );
        }
    }
}
