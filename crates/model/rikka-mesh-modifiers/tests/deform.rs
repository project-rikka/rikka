use glam::{Mat4, Vec3};
use rikka_mesh_modifiers::{
    Context, CorrectiveSmooth, Lattice, LatticeGrid, LatticeInterpolation, ModifierKind,
    RestSource, SkipReason, SmoothType, Standalone, WeightedNormal, WeightedNormalMode,
};
use rikka_polymesh::{Domain, Polymesh};

use crate::compare::NORMAL_TOLERANCE;
use crate::oracle::deform::{
    LATTICE_CARDINAL, LATTICE_GROUP, LATTICE_MASK, LATTICE_MESH, LATTICE_MESH_WORLD, LATTICE_MIXED,
    LATTICE_POINTS, LATTICE_REST, LATTICE_WORLD, SMOOTH_BENT, SMOOTH_GROUP_INVERTED,
    SMOOTH_LENGTH_PINNED, SMOOTH_MESH, SMOOTH_ONLY, SMOOTH_SIMPLE, SMOOTH_WEIGHTS,
    WEIGHTED_ANGLE_KEEP_SHARP, WEIGHTED_COMBINED_STRENGTH, WEIGHTED_FACE_AREA, WEIGHTED_GROUP,
    WEIGHTED_GROUP_KEEP, WEIGHTED_MESH, WEIGHTED_SHARP_EDGES, WEIGHTED_STRENGTHS,
};
use crate::support::{assert_close, fixture, run, run_in};

const FACE_STRENGTH: &str = "__mod_weightednormals_faceweight";
const DEFORM_TOLERANCE: f32 = 2.0e-5;

struct Rest {
    positions: Vec<Vec3>,
}

impl Context for Rest {
    fn object_matrix(&self) -> Mat4 {
        Mat4::IDENTITY
    }

    fn original_positions(&self) -> Option<&[Vec3]> {
        Some(&self.positions)
    }

    fn material_count(&self) -> usize {
        0
    }
}

fn assert_near(actual: &[Vec3], expected: &[[f32; 3]], tolerance: f32) {
    assert_eq!(actual.len(), expected.len(), "count");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let expected = Vec3::from(*expected);
        assert!(
            actual.abs_diff_eq(expected, tolerance),
            "element {index}: {actual} != {expected}"
        );
    }
}

fn weighted_mesh() -> Polymesh {
    let mut mesh = fixture(&WEIGHTED_MESH);
    let group = mesh.vertex_groups.add("Group");
    mesh.vertex_groups.ensure_weights(mesh.point_count());
    for &point in WEIGHTED_GROUP {
        mesh.vertex_groups.set_weight(point as usize, group, 0.5);
    }
    mesh
}

#[test]
fn weighted_normal_matches_blender() {
    let cases: [(WeightedNormal, bool, bool, &[[f32; 3]]); 4] = [
        (WeightedNormal::default(), false, false, WEIGHTED_FACE_AREA),
        (
            WeightedNormal {
                mode: WeightedNormalMode::CornerAngle,
                weight: 70,
                keep_sharp: true,
                ..WeightedNormal::default()
            },
            true,
            false,
            WEIGHTED_ANGLE_KEEP_SHARP,
        ),
        (
            WeightedNormal {
                mode: WeightedNormalMode::FaceAreaAngle,
                weight: 30,
                threshold: 0.05,
                use_face_influence: true,
                ..WeightedNormal::default()
            },
            false,
            true,
            WEIGHTED_COMBINED_STRENGTH,
        ),
        (
            WeightedNormal {
                weight: 90,
                keep_sharp: true,
                vertex_group: "Group".to_owned(),
                ..WeightedNormal::default()
            },
            false,
            false,
            WEIGHTED_GROUP_KEEP,
        ),
    ];
    for (settings, sharp, strengths, expected) in cases {
        let mut mesh = weighted_mesh();
        if sharp {
            let edges = mesh.sharp_edges_mut();
            for &edge in WEIGHTED_SHARP_EDGES {
                edges[edge as usize] = true;
            }
        }
        if strengths {
            mesh.attributes
                .ensure::<i32>(FACE_STRENGTH, Domain::Face, WEIGHTED_STRENGTHS.len())
                .copy_from_slice(WEIGHTED_STRENGTHS);
        }
        let evaluated = run(&mesh, settings);
        let normals = evaluated.mesh.custom_normals().expect("custom normals");
        assert_near(normals, expected, 2.0 * NORMAL_TOLERANCE);
    }
}

fn bent() -> (Polymesh, Rest) {
    let mut mesh = fixture(&SMOOTH_MESH);
    let rest = Rest {
        positions: mesh.positions.clone(),
    };
    mesh.positions = SMOOTH_BENT.iter().copied().map(Vec3::from).collect();
    let group = mesh.vertex_groups.add("Top");
    mesh.vertex_groups.ensure_weights(mesh.point_count());
    for (point, &weight) in SMOOTH_WEIGHTS.iter().enumerate() {
        mesh.vertex_groups.set_weight(point, group, weight);
    }
    (mesh, rest)
}

#[test]
fn corrective_smooth_matches_blender() {
    let cases: [(CorrectiveSmooth, &[[f32; 3]]); 4] = [
        (CorrectiveSmooth::default(), SMOOTH_SIMPLE),
        (
            CorrectiveSmooth {
                smooth_type: SmoothType::LengthWeighted,
                factor: 0.6,
                iterations: 10,
                scale: 0.8,
                use_pin_boundary: true,
                ..CorrectiveSmooth::default()
            },
            SMOOTH_LENGTH_PINNED,
        ),
        (
            CorrectiveSmooth {
                factor: 0.4,
                iterations: 4,
                vertex_group: "Top".to_owned(),
                invert_vertex_group: true,
                ..CorrectiveSmooth::default()
            },
            SMOOTH_GROUP_INVERTED,
        ),
        (
            CorrectiveSmooth {
                iterations: 3,
                use_only_smooth: true,
                ..CorrectiveSmooth::default()
            },
            SMOOTH_ONLY,
        ),
    ];
    for (settings, expected) in cases {
        let (mesh, rest) = bent();
        let evaluated = run_in(&mesh, settings, &rest);
        assert_near(&evaluated.mesh.positions, expected, DEFORM_TOLERANCE);
    }
}

#[test]
fn corrective_smooth_bind_coordinates_stand_in_for_the_original() {
    let (mesh, rest) = bent();
    let bound = CorrectiveSmooth {
        rest_source: RestSource::Bind,
        bind_coords: rest.positions.clone(),
        ..CorrectiveSmooth::default()
    };
    let evaluated = run_in(&mesh, bound, &Standalone::default());
    assert_near(&evaluated.mesh.positions, SMOOTH_SIMPLE, DEFORM_TOLERANCE);
}

#[test]
fn corrective_smooth_reports_missing_or_mismatched_rest() {
    let (mesh, _) = bent();
    let unbound = CorrectiveSmooth {
        rest_source: RestSource::Bind,
        ..CorrectiveSmooth::default()
    };
    let stale = CorrectiveSmooth {
        rest_source: RestSource::Bind,
        bind_coords: vec![Vec3::ZERO; 3],
        ..CorrectiveSmooth::default()
    };
    for (settings, reason) in [
        (unbound, SkipReason::BindRequired),
        (stale, SkipReason::VertexCountMismatch),
    ] {
        let stack = [rikka_mesh_modifiers::Modifier::new("CS", settings)];
        let evaluated = rikka_mesh_modifiers::evaluate(
            &mesh,
            &stack,
            &Standalone::default(),
            rikka_mesh_modifiers::Purpose::Viewport,
        );
        assert_eq!(
            evaluated.notes[0].kind,
            rikka_mesh_modifiers::NoteKind::Skipped(reason)
        );
        assert_eq!(evaluated.mesh.positions, mesh.positions);
    }
}

fn lattice_grid(interpolation: [LatticeInterpolation; 3]) -> LatticeGrid {
    let transform = Mat4::from_cols_array(&LATTICE_MESH_WORLD).inverse()
        * Mat4::from_cols_array(&LATTICE_WORLD);
    let mut grid = LatticeGrid::new([3, 2, 4], transform);
    let rest: Vec<Vec3> = (0..grid.point_count())
        .map(|index| grid.rest_point(grid.cell(index)))
        .collect();
    assert_close(&rest, LATTICE_REST);
    grid.points_mut()
        .iter_mut()
        .zip(LATTICE_POINTS)
        .for_each(|(point, &expected)| *point = Vec3::from(expected));
    grid.interpolation = interpolation;
    grid
}

#[test]
fn lattice_matches_blender() {
    use LatticeInterpolation::{BSpline, Cardinal, CatmullRom, Linear};
    let cases: [(Lattice, &[[f32; 3]]); 3] = [
        (
            Lattice {
                lattice: lattice_grid([BSpline, Linear, CatmullRom]),
                ..Lattice::default()
            },
            LATTICE_MIXED,
        ),
        (
            Lattice {
                lattice: lattice_grid([BSpline; 3]),
                strength: 0.7,
                vertex_group: "Mask".to_owned(),
                ..Lattice::default()
            },
            LATTICE_GROUP,
        ),
        (
            Lattice {
                lattice: lattice_grid([Cardinal; 3]),
                ..Lattice::default()
            },
            LATTICE_CARDINAL,
        ),
    ];
    for (settings, expected) in cases {
        let mut mesh = fixture(&LATTICE_MESH);
        let group = mesh.vertex_groups.add("Mask");
        mesh.vertex_groups.ensure_weights(mesh.point_count());
        for (point, &weight) in LATTICE_MASK.iter().enumerate() {
            mesh.vertex_groups.set_weight(point, group, weight);
        }
        let evaluated = run(&mesh, settings);
        assert_near(&evaluated.mesh.positions, expected, DEFORM_TOLERANCE);
    }
}

#[test]
fn lattice_resize_keeps_the_deformed_shape() {
    let mut grid = LatticeGrid::new([2, 2, 2], Mat4::IDENTITY);
    grid.interpolation = [LatticeInterpolation::Linear; 3];
    for point in grid.points_mut() {
        *point *= Vec3::new(1.5, 1.0, 0.5);
    }
    let resized = grid.resized([4, 3, 2]);
    let mesh = fixture(&LATTICE_MESH);
    let deform = |grid: LatticeGrid| {
        let kind: ModifierKind = Lattice {
            lattice: grid,
            ..Lattice::default()
        }
        .into();
        run(&mesh, kind).mesh.positions
    };
    let before = deform(grid);
    let after = deform(resized);
    for (a, b) in before.iter().zip(&after) {
        assert!(a.abs_diff_eq(*b, 1.0e-5), "{a} != {b}");
    }
}
