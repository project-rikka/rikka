use rikka_mesh_modifiers::{BoundaryShape, Solidify, SolidifyMode, ThicknessMode};

use crate::compare::{MeshData, assert_mesh, assert_order};
use crate::oracle::basic::{
    SOLIDIFY_CYLINDER_CLAMP, SOLIDIFY_CYLINDER_EVEN_HQ, SOLIDIFY_GRID, SOLIDIFY_RIM_ONLY,
};
use crate::oracle::solidify::{
    BOWTIE_INPUT, CELLS_INPUT, COMPLEX_BOWTIE, COMPLEX_CELLS, COMPLEX_CELLS_EVEN, COMPLEX_CROSS,
    COMPLEX_CROSS_ROUND, COMPLEX_GRID, COMPLEX_GRID_CLAMP, COMPLEX_GRID_FLAT,
    COMPLEX_GRID_FLAT_FACES, COMPLEX_GRID_GROUP, COMPLEX_GRID_ROUND, COMPLEX_MERGE, COMPLEX_MOBIUS,
    COMPLEX_TEE, COMPLEX_TEE_EVEN, COMPLEX_TEE_RIM_ONLY, CROSS_INPUT, GRID_GROUPS_INPUT,
    GRID_INPUT, MERGE_INPUT, MOBIUS_INPUT, TEE_INPUT,
};
use crate::support::{
    CYLINDER, GRID, Oracle, Scene, assert_edges, assert_matches, fixture, run, run_in,
};

const GROUPS: &[&str] = &["Shell", "Rim", "Weight"];

fn complex(thickness: f32) -> Solidify {
    Solidify {
        solidify_mode: SolidifyMode::Complex,
        thickness,
        ..Solidify::default()
    }
}

#[test]
fn complex_solidify_matches_blender() {
    let weighted = Solidify {
        vertex_group: String::from("Weight"),
        invert_vertex_group: true,
        thickness_vertex_group: 0.3,
        nonmanifold_boundary_mode: BoundaryShape::Round,
        ..complex(0.2)
    };
    let cases: [(&MeshData, &[&str], Solidify, &MeshData); 16] = [
        (&GRID_INPUT, &[], complex(0.1), &COMPLEX_GRID),
        (
            &GRID_GROUPS_INPUT,
            GROUPS,
            Solidify {
                offset: 0.0,
                nonmanifold_thickness_mode: ThicknessMode::Fixed,
                nonmanifold_boundary_mode: BoundaryShape::Round,
                material_offset: 1,
                material_offset_rim: 2,
                shell_vertex_group: String::from("Shell"),
                rim_vertex_group: String::from("Rim"),
                ..complex(0.2)
            },
            &COMPLEX_GRID_ROUND,
        ),
        (
            &GRID_INPUT,
            &[],
            Solidify {
                offset: 0.5,
                nonmanifold_thickness_mode: ThicknessMode::Even,
                nonmanifold_boundary_mode: BoundaryShape::Flat,
                use_flip_normals: true,
                ..complex(-0.15)
            },
            &COMPLEX_GRID_FLAT,
        ),
        (
            &GRID_GROUPS_INPUT,
            GROUPS,
            weighted.clone(),
            &COMPLEX_GRID_GROUP,
        ),
        (
            &GRID_GROUPS_INPUT,
            GROUPS,
            Solidify {
                use_flat_faces: true,
                ..weighted
            },
            &COMPLEX_GRID_FLAT_FACES,
        ),
        (
            &GRID_INPUT,
            &[],
            Solidify {
                thickness_clamp: 0.6,
                use_rim: false,
                ..complex(0.9)
            },
            &COMPLEX_GRID_CLAMP,
        ),
        (&TEE_INPUT, &[], complex(0.1), &COMPLEX_TEE),
        (
            &TEE_INPUT,
            &[],
            Solidify {
                nonmanifold_thickness_mode: ThicknessMode::Fixed,
                use_rim_only: true,
                ..complex(0.1)
            },
            &COMPLEX_TEE_RIM_ONLY,
        ),
        (
            &TEE_INPUT,
            &[],
            Solidify {
                offset: 0.3,
                nonmanifold_thickness_mode: ThicknessMode::Even,
                nonmanifold_boundary_mode: BoundaryShape::Flat,
                ..complex(0.12)
            },
            &COMPLEX_TEE_EVEN,
        ),
        (
            &CELLS_INPUT,
            &[],
            Solidify {
                bevel_convex: 0.5,
                ..complex(0.2)
            },
            &COMPLEX_CELLS,
        ),
        (
            &CELLS_INPUT,
            &[],
            Solidify {
                nonmanifold_thickness_mode: ThicknessMode::Even,
                thickness_clamp: 0.5,
                use_thickness_angle_clamp: true,
                bevel_convex: -0.4,
                ..complex(0.3)
            },
            &COMPLEX_CELLS_EVEN,
        ),
        (
            &MERGE_INPUT,
            &[],
            Solidify {
                nonmanifold_merge_threshold: 0.001,
                nonmanifold_boundary_mode: BoundaryShape::Flat,
                ..complex(0.1)
            },
            &COMPLEX_MERGE,
        ),
        (&BOWTIE_INPUT, &[], complex(0.1), &COMPLEX_BOWTIE),
        (&MOBIUS_INPUT, &[], complex(0.05), &COMPLEX_MOBIUS),
        (&CROSS_INPUT, &[], complex(0.1), &COMPLEX_CROSS),
        (
            &CROSS_INPUT,
            &[],
            Solidify {
                nonmanifold_boundary_mode: BoundaryShape::Round,
                nonmanifold_thickness_mode: ThicknessMode::Fixed,
                ..complex(0.1)
            },
            &COMPLEX_CROSS_ROUND,
        ),
    ];
    for (input, groups, solidify, expected) in cases {
        let scene = Scene {
            materials: match groups.is_empty() {
                true => 0,
                false => 3,
            },
            ..Scene::default()
        };
        let evaluated = run_in(&input.build(groups), solidify, &scene).mesh;
        assert_mesh(&evaluated, expected);
        assert_order(&evaluated, expected);
    }
}

#[test]
fn simple_solidify_matches_blender() {
    let cases: [(Solidify, bool, &Oracle); 4] = [
        (
            Solidify {
                thickness: 0.1,
                ..Solidify::default()
            },
            false,
            &SOLIDIFY_GRID,
        ),
        (
            Solidify {
                thickness: 0.2,
                offset: 0.3,
                use_even_offset: true,
                use_quality_normals: true,
                ..Solidify::default()
            },
            true,
            &SOLIDIFY_CYLINDER_EVEN_HQ,
        ),
        (
            Solidify {
                thickness: 0.8,
                thickness_clamp: 0.5,
                use_thickness_angle_clamp: true,
                ..Solidify::default()
            },
            true,
            &SOLIDIFY_CYLINDER_CLAMP,
        ),
        (
            Solidify {
                thickness: 0.1,
                use_rim_only: true,
                use_flip_normals: true,
                ..Solidify::default()
            },
            false,
            &SOLIDIFY_RIM_ONLY,
        ),
    ];
    for (solidify, cylinder, oracle) in cases {
        let mesh = match cylinder {
            true => fixture(&CYLINDER),
            false => fixture(&GRID),
        };
        let evaluated = run(&mesh, solidify).mesh;
        assert_matches(&evaluated, oracle);
        assert_edges(&evaluated, oracle);
    }
}
