use rikka_mesh_modifiers::{
    BoundarySmooth, Modifier, Purpose, StackCache, Subdivision, SubdivisionType, UvSmooth,
    evaluate, evaluate_cached,
};
use rikka_polymesh::Polymesh;

use crate::compare::{MeshData, assert_order};
use crate::oracle::basic::{
    SUBSURF_CREASE, SUBSURF_CUBE_1, SUBSURF_CUBE_2_NO_LIMIT, SUBSURF_GRID_BOUNDARY_CORNERS,
    SUBSURF_GRID_UV_ALL, SUBSURF_GRID_UV_BOUNDARIES, SUBSURF_SIMPLE,
};
use crate::oracle::subsurf::{
    SUBSURF_NGON_2, SUBSURF_NGON_3, SUBSURF_NGON_INPUT, SUBSURF_NGON_SIMPLE_2,
};
use crate::support::{
    CUBE, GRID, Oracle, assert_edges, assert_matches, assert_uvs, fixture, run, with_uvs,
};

#[test]
fn subdivision_matches_blender() {
    let cases: [(&str, Subdivision, &Oracle); 4] = [
        ("limit", Subdivision::default(), &SUBSURF_CUBE_1),
        (
            "recursive",
            Subdivision {
                levels: 2,
                use_limit_surface: false,
                ..Subdivision::default()
            },
            &SUBSURF_CUBE_2_NO_LIMIT,
        ),
        (
            "simple",
            Subdivision {
                subdivision_type: SubdivisionType::Simple,
                ..Subdivision::default()
            },
            &SUBSURF_SIMPLE,
        ),
        (
            "boundary corners",
            Subdivision {
                boundary_smooth: BoundarySmooth::PreserveCorners,
                ..Subdivision::default()
            },
            &SUBSURF_GRID_BOUNDARY_CORNERS,
        ),
    ];
    for (label, subdivision, oracle) in cases {
        let mesh = match label {
            "boundary corners" => fixture(&GRID),
            _ => fixture(&CUBE),
        };
        let evaluated = run(&mesh, subdivision).mesh;
        assert_matches(&evaluated, oracle);
        assert_edges(&evaluated, oracle);
    }
}

#[test]
fn deep_subdivision_follows_blender_element_order() {
    let cases: [(u8, SubdivisionType, &MeshData); 3] = [
        (2, SubdivisionType::CatmullClark, &SUBSURF_NGON_2),
        (3, SubdivisionType::CatmullClark, &SUBSURF_NGON_3),
        (2, SubdivisionType::Simple, &SUBSURF_NGON_SIMPLE_2),
    ];
    for (levels, subdivision_type, expected) in cases {
        let subdivision = Subdivision {
            levels,
            subdivision_type,
            use_limit_surface: false,
            ..Subdivision::default()
        };
        let evaluated = run(&SUBSURF_NGON_INPUT.build(&[]), subdivision).mesh;
        assert_order(&evaluated, expected);
    }
}

#[test]
fn creases_keep_edges_sharp() {
    let mut cube = fixture(&CUBE);
    let creases = cube.edge_creases_mut();
    creases[..4].fill(1.0);
    assert_matches(&run(&cube, Subdivision::default()).mesh, &SUBSURF_CREASE);
}

#[test]
fn uv_smoothing_matches_blender() {
    for (smooth, oracle) in [
        (UvSmooth::All, &SUBSURF_GRID_UV_ALL),
        (UvSmooth::PreserveBoundaries, &SUBSURF_GRID_UV_BOUNDARIES),
    ] {
        let grid = with_uvs(fixture(&GRID));
        let subdivision = Subdivision {
            uv_smooth: smooth,
            ..Subdivision::default()
        };
        let evaluated = run(&grid, subdivision).mesh;
        assert_matches(&evaluated, oracle);
        assert_uvs(&evaluated, "UVMap", oracle.uvs);
    }
}

#[test]
fn render_purpose_uses_render_levels() {
    let cube = fixture(&CUBE);
    let stack = [rikka_mesh_modifiers::Modifier::new(
        "Subdivision",
        Subdivision {
            levels: 1,
            render_levels: 2,
            ..Subdivision::default()
        },
    )];
    let scene = crate::support::Scene::default();
    let viewport = rikka_mesh_modifiers::evaluate(
        &cube,
        &stack,
        &scene,
        rikka_mesh_modifiers::Purpose::Viewport,
    );
    let render = rikka_mesh_modifiers::evaluate(
        &cube,
        &stack,
        &scene,
        rikka_mesh_modifiers::Purpose::Render,
    );
    assert_eq!(viewport.mesh.face_count(), 24);
    assert_eq!(render.mesh.face_count(), 96);
}

#[test]
fn cached_evaluation_matches_fresh_evaluation() {
    let stack = [Modifier::new(
        "Subdivision",
        Subdivision {
            levels: 2,
            ..Subdivision::default()
        },
    )];
    let scene = crate::support::Scene::default();
    let mut cache = StackCache::new();
    let mut grid = with_uvs(fixture(&GRID));
    let moved = |mut mesh: Polymesh, offset: f32| {
        for position in mesh.positions.iter_mut().step_by(3) {
            position.z += offset;
        }
        mesh
    };
    let mut extra = grid.clone();
    extra.retain_faces(&[false]);
    for step in 0..4 {
        grid = match step {
            2 => extra.clone(),
            _ => moved(grid, 0.25),
        };
        let cached = evaluate_cached(&grid, &stack, &scene, Purpose::Viewport, &mut cache);
        let fresh = evaluate(&grid, &stack, &scene, Purpose::Viewport);
        assert_eq!(cached, fresh, "step {step}");
    }
}
