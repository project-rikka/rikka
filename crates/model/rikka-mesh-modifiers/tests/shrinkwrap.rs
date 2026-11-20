use glam::Vec3;
use rikka_mesh_modifiers::{CullFace, ObjectRef, Shrinkwrap, WrapMethod, WrapMode};
use rikka_polymesh::Polymesh;

use crate::oracle::scene::{
    SHRINK_ABOVE, SHRINK_GROUP_INVERT, SHRINK_INSIDE_OFFSET, SHRINK_NEAREST, SHRINK_OUTSIDE_OFFSET,
    SHRINK_PROJECT_LIMIT, SHRINK_PROJECT_NORMAL_BOTH, SHRINK_PROJECT_Z, SHRINK_TARGET_PROJECT,
    SHRINK_VERTEX,
};
use crate::support::{CUBE, GRID, Probe, Scene, assert_close, build, fixture, placement, run_in};

fn sheet() -> Polymesh {
    let positions = (0..9)
        .map(|index| {
            let (x, y) = ((index % 3) as f32, (index / 3) as f32);
            Vec3::new(x * 1.5 - 1.0, y * 1.5 - 1.0, 1.5 + 0.1 * x)
        })
        .collect();
    let mut mesh = build(positions, GRID.edges, GRID.faces);
    let mask = mesh.vertex_groups.add("Mask");
    mesh.vertex_groups.ensure_weights(9);
    for point in 0..9 {
        mesh.vertex_groups
            .set_weight(point, mask, (point % 4) as f32 / 3.0);
    }
    mesh
}

fn scene() -> Scene {
    Scene {
        matrix: placement([0.1, 0.0, 0.2], [0.0, 0.1, 0.0], [1.0; 3]),
        ..Scene::default()
    }
    .with_object(
        placement([0.2, 0.3, -0.4], [0.3, 0.2, 0.1], [1.2, 0.8, 1.0]),
        Some(fixture(&CUBE)),
    )
}

#[test]
fn shrinkwrap_matches_blender() {
    let base = Shrinkwrap {
        target: Some(ObjectRef::new(0)),
        ..Shrinkwrap::default()
    };
    let project_z = Shrinkwrap {
        wrap_method: WrapMethod::Project,
        use_project: [false, false, true],
        use_negative_direction: true,
        ..base.clone()
    };
    let cases: [(Shrinkwrap, &Probe); 10] = [
        (base.clone(), &SHRINK_NEAREST),
        (
            Shrinkwrap {
                wrap_mode: WrapMode::Outside,
                offset: 0.1,
                ..base.clone()
            },
            &SHRINK_OUTSIDE_OFFSET,
        ),
        (
            Shrinkwrap {
                wrap_mode: WrapMode::Inside,
                offset: 0.2,
                ..base.clone()
            },
            &SHRINK_INSIDE_OFFSET,
        ),
        (
            Shrinkwrap {
                wrap_mode: WrapMode::AboveSurface,
                offset: 0.15,
                ..base.clone()
            },
            &SHRINK_ABOVE,
        ),
        (
            Shrinkwrap {
                wrap_method: WrapMethod::NearestVertex,
                offset: 0.05,
                ..base.clone()
            },
            &SHRINK_VERTEX,
        ),
        (
            Shrinkwrap {
                use_positive_direction: false,
                ..project_z.clone()
            },
            &SHRINK_PROJECT_Z,
        ),
        (
            Shrinkwrap {
                wrap_method: WrapMethod::Project,
                use_negative_direction: true,
                cull_face: CullFace::Back,
                offset: 0.02,
                ..base.clone()
            },
            &SHRINK_PROJECT_NORMAL_BOTH,
        ),
        (
            Shrinkwrap {
                project_limit: 1.2,
                ..project_z
            },
            &SHRINK_PROJECT_LIMIT,
        ),
        (
            Shrinkwrap {
                wrap_method: WrapMethod::TargetProject,
                wrap_mode: WrapMode::AboveSurface,
                offset: 0.05,
                ..base.clone()
            },
            &SHRINK_TARGET_PROJECT,
        ),
        (
            Shrinkwrap {
                vertex_group: String::from("Mask"),
                invert_vertex_group: true,
                ..base
            },
            &SHRINK_GROUP_INVERT,
        ),
    ];
    let scene = scene();
    for (index, (shrinkwrap, probe)) in cases.into_iter().enumerate() {
        let evaluated = run_in(&sheet(), shrinkwrap, &scene).mesh;
        eprintln!("case {index}");
        assert_close(&evaluated.positions, probe.positions);
    }
}
