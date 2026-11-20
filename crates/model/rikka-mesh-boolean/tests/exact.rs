use std::f32::consts::FRAC_PI_2;

use glam::Mat4;
use rikka_mesh_boolean::{Boolean, Operand, Operation, Solver};
use rikka_polymesh::Polymesh;

use crate::support::{
    Expected, assert_stats, cube, expect, grid, moved, placement, torus, uv_sphere,
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

fn run(a: &Polymesh, b: &Polymesh, transform: Mat4, operation: Operation) -> Polymesh {
    Boolean::new(operation)
        .operand(a)
        .operand(Operand::new(b).transform(transform))
        .solver(Solver::Exact)
        .compute()
        .unwrap()
        .mesh
}

fn check_case(label: &str, a: &Polymesh, b: &Polymesh, transform: Mat4, expected: [Expected; 3]) {
    let operations = [
        Operation::Intersect,
        Operation::Union,
        Operation::Difference,
    ];
    for (operation, expected) in operations.into_iter().zip(expected) {
        let mesh = run(a, b, transform, operation);
        assert_stats(&mesh, expected, &format!("{label} {operation:?}"));
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
fn coplanar_cubes_match_blender() {
    check_case(
        "coplanar",
        &cube(),
        &cube(),
        moved([1.0, 0.0, 0.0]),
        [
            expect(8, 12, 6, 4.0, 16.0, true, 6),
            expect(16, 28, 14, 12.0, 32.0, true, 14),
            expect(8, 12, 6, 4.0, 16.0, true, 6),
        ],
    );
}

#[test]
fn cubes_sharing_edge_match_blender() {
    check_case(
        "shared edge",
        &cube(),
        &cube(),
        moved([2.0, 2.0, 0.0]),
        [
            EMPTY,
            expect(14, 23, 12, 16.0, 48.0, false, 12),
            expect(8, 12, 6, 8.0, 24.0, true, 6),
        ],
    );
}

#[test]
fn cubes_sharing_vertex_keep_both_volumes() {
    check_case(
        "shared vertex",
        &cube(),
        &cube(),
        moved([2.0, 2.0, 2.0]),
        [
            EMPTY,
            expect(15, 24, 12, 16.0, 48.0, true, 12),
            expect(8, 12, 6, 8.0, 24.0, true, 6),
        ],
    );
}

#[test]
fn identical_cubes_match_blender() {
    check_case(
        "identical",
        &cube(),
        &cube(),
        Mat4::IDENTITY,
        [
            expect(8, 12, 6, 8.0, 24.0, true, 6),
            expect(8, 12, 6, 8.0, 24.0, true, 6),
            EMPTY,
        ],
    );
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
            expect(274, 529, 257, 2.651_695_2, 10.284_384_8, true, 200),
            expect(348, 673, 327, 9.470_243_5, 26.181_302_7, true, 267),
            expect(282, 541, 263, 5.348_304_8, 24.358_019_6, true, 205),
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
fn open_operand_matches_blender() {
    let transform = placement([0.0, 0.0, 0.3], [0.2, 0.1, 0.0], 1.0);
    let expected = [
        expect(25, 40, 16, 0.400_000_01, 4.101_847_4, false, 16),
        expect(36, 52, 17, 3.677_653_5, 14.498_154_6, false, 9),
        expect(29, 48, 21, 2.799_999_96, 13.701_847_3, true, 17),
    ];
    let operations = [
        Operation::Intersect,
        Operation::Union,
        Operation::Difference,
    ];
    for hole_tolerant in [false, true] {
        for (operation, expected) in operations.into_iter().zip(expected) {
            let mesh = Boolean::new(operation)
                .operand(&cube())
                .operand(Operand::new(&grid()).transform(transform))
                .hole_tolerant(hole_tolerant)
                .compute()
                .unwrap()
                .mesh;
            assert_stats(
                &mesh,
                expected,
                &format!("open {operation:?} hole tolerant {hole_tolerant}"),
            );
        }
    }
}

fn overlapping_pair() -> Polymesh {
    let mut mesh = cube();
    let mut second = cube();
    second.transform(&moved([1.0, 0.7, 0.4]));
    mesh.append(&second);
    mesh
}

#[test]
fn self_intersecting_operand_matches_blender() {
    let a = overlapping_pair();
    let b = cube();
    let transform = moved([-1.2, -0.3, 0.2]);
    let cases = [
        (
            Operation::Union,
            true,
            expect(32, 48, 18, 19.472, 50.32, true, 7),
        ),
        (
            Operation::Difference,
            true,
            expect(26, 39, 15, 11.472, 38.04, true, 7),
        ),
        (
            Operation::Union,
            false,
            expect(22, 33, 15, 2.448, 48.0, true, 12),
        ),
        (
            Operation::Difference,
            false,
            expect(28, 42, 18, -5.552, 60.28, true, 12),
        ),
    ];
    for (operation, use_self, expected) in cases {
        let mesh = Boolean::new(operation)
            .operand(&a)
            .operand(Operand::new(&b).transform(transform))
            .self_intersection(use_self)
            .compute()
            .unwrap()
            .mesh;
        assert_stats(
            &mesh,
            expected,
            &format!("self {operation:?} use_self {use_self}"),
        );
    }
}

fn ring_transforms() -> Vec<Mat4> {
    (0..5)
        .map(|k| {
            let angle = k as f64 * 1.2566;
            moved([
                (1.3 * angle.cos()) as f32,
                (1.3 * angle.sin()) as f32,
                (0.15 * k as f64) as f32,
            ])
        })
        .collect()
}

#[test]
fn many_operand_boolean_matches_blender() {
    let a = cube();
    let b = cube();
    let cases = [
        (
            Operation::Union,
            expect(72, 110, 40, 33.579_746_3, 72.810_048_2, true, 14),
        ),
        (
            Operation::Difference,
            expect(28, 42, 16, 1.149_040_2, 10.726_233_0, true, 7),
        ),
        (Operation::Intersect, EMPTY),
    ];
    for (operation, expected) in cases {
        let mesh = Boolean::new(operation)
            .operand(&a)
            .operands(
                ring_transforms()
                    .into_iter()
                    .map(|transform| Operand::new(&b).transform(transform)),
            )
            .compute()
            .unwrap()
            .mesh;
        assert_stats(&mesh, expected, &format!("many {operation:?}"));
    }
}

#[test]
fn near_degenerate_operands_stay_closed() {
    let cases = [
        (
            "tiny rotation",
            placement([0.0; 3], [0.0, 0.0, 1.0e-6], 1.0),
            8.0,
        ),
        ("tiny offset", moved([1.0e-7, 0.0, 0.0]), 8.0),
        (
            "tilted face",
            placement([0.3, 0.2, 2.0], [1.0e-5, 2.0e-5, 0.0], 1.0),
            16.0,
        ),
        (
            "vertex on face",
            placement(
                [0.0, 0.0, 2.0],
                [0.0, 0.0, std::f32::consts::FRAC_PI_4],
                1.0,
            ),
            16.0,
        ),
    ];
    for (label, transform, volume) in cases {
        let mesh = run(&cube(), &cube(), transform, Operation::Union);
        mesh.validate().unwrap();
        let summary = crate::support::stats(&mesh);
        assert!(summary.manifold, "{label}: not closed");
        assert!(
            (summary.volume - volume).abs() < 1.0e-3,
            "{label}: volume {}",
            summary.volume
        );
    }
}

#[test]
fn single_operand_self_union_removes_overlap() {
    let mesh = Boolean::new(Operation::Union)
        .operand(&overlapping_pair())
        .self_intersection(true)
        .compute()
        .unwrap()
        .mesh;
    crate::support::assert_shape(&mesh, 13.92, 38.04, true, "self union");
}
