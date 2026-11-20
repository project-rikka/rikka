use glam::Vec2;
use rikka_mesh_modifiers::{
    Bevel, BevelAffect, BevelFaceStrength, BevelLimitMethod, BevelMiter, BevelOffsetType,
    BevelProfileType, BevelVertexMesh, CustomProfile, Modifier, NoteKind, ProfileHandle,
    ProfilePoint, Purpose, SkipReason, evaluate,
};

use crate::compare::{MeshData, assert_corner_normals, assert_face_values, assert_mesh};
use crate::oracle::bevel::{
    BEVEL_ANGLE, BEVEL_CUBE, BEVEL_CUSTOM, BEVEL_CUSTOM_EVEN, BEVEL_DATA, BEVEL_HARDEN,
    BEVEL_HARDEN_NORMALS, BEVEL_HARDEN_STRENGTH, BEVEL_MITERS, BEVEL_PROFILE, BEVEL_VERTICES,
    BEVEL_VERTICES_INVERTED, BEVEL_WEIGHTS, CUBE_DATA_INPUT, CUBE_GROUP_INPUT, CUBE_INPUT,
    CUBE_WEIGHTS_INPUT, L_INPUT, PRISM_INPUT,
};
use crate::support::{Scene, run, run_in};

const FACE_STRENGTH: &str = "__mod_weightednormals_faceweight";

fn custom_profile(even: bool) -> CustomProfile {
    let handle = |name: &str| match name {
        "Auto" => ProfileHandle::Auto,
        "Vector" => ProfileHandle::Vector,
        "Free" => ProfileHandle::Free,
        _ => ProfileHandle::Aligned,
    };
    CustomProfile {
        points: BEVEL_PROFILE
            .iter()
            .map(|&(location, left, right)| ProfilePoint {
                location: Vec2::from(location),
                handle_left_type: handle(left),
                handle_right_type: handle(right),
                handle_left: Vec2::from(location),
                handle_right: Vec2::from(location),
            })
            .collect(),
        use_sample_straight_edges: even,
        use_sample_even_lengths: even,
        ..CustomProfile::default()
    }
}

#[test]
fn bevel_matches_blender() {
    let custom = |even: bool| Bevel {
        profile_type: BevelProfileType::Custom,
        segments: 4,
        width: 0.3,
        custom_profile: custom_profile(even),
        ..Bevel::default()
    };
    let cases: [(&MeshData, &[&str], Bevel, &MeshData); 8] = [
        (&CUBE_INPUT, &[], Bevel::default(), &BEVEL_CUBE),
        (
            &CUBE_GROUP_INPUT,
            &["Corner"],
            Bevel {
                affect: BevelAffect::Vertices,
                limit_method: BevelLimitMethod::VertexGroup,
                vertex_group: String::from("Corner"),
                segments: 2,
                offset_type: BevelOffsetType::Width,
                width: 0.4,
                ..Bevel::default()
            },
            &BEVEL_VERTICES,
        ),
        (
            &CUBE_GROUP_INPUT,
            &["Corner"],
            Bevel {
                affect: BevelAffect::Vertices,
                limit_method: BevelLimitMethod::VertexGroup,
                vertex_group: String::from("Corner"),
                invert_vertex_group: true,
                offset_type: BevelOffsetType::Width,
                width: 0.4,
                ..Bevel::default()
            },
            &BEVEL_VERTICES_INVERTED,
        ),
        (&CUBE_INPUT, &[], custom(false), &BEVEL_CUSTOM),
        (&CUBE_INPUT, &[], custom(true), &BEVEL_CUSTOM_EVEN),
        (
            &L_INPUT,
            &[],
            Bevel {
                segments: 2,
                width: 0.2,
                miter_outer: BevelMiter::Patch,
                miter_inner: BevelMiter::Arc,
                spread: 0.05,
                vmesh_method: BevelVertexMesh::Cutoff,
                limit_method: BevelLimitMethod::None,
                ..Bevel::default()
            },
            &BEVEL_MITERS,
        ),
        (
            &PRISM_INPUT,
            &[],
            Bevel {
                angle_limit: 60.0f32.to_radians(),
                width: 0.3,
                use_clamp_overlap: false,
                loop_slide: false,
                offset_type: BevelOffsetType::Depth,
                ..Bevel::default()
            },
            &BEVEL_ANGLE,
        ),
        (
            &CUBE_DATA_INPUT,
            &["Side"],
            Bevel {
                segments: 2,
                width: 0.2,
                ..Bevel::default()
            },
            &BEVEL_DATA,
        ),
    ];
    for (input, groups, bevel, expected) in cases {
        let evaluated = run(&input.build(groups), bevel);
        assert_mesh(&evaluated.mesh, expected);
    }
}

#[test]
fn bevel_weights_marks_and_material_match_blender() {
    let scene = Scene {
        materials: 2,
        ..Scene::default()
    };
    let bevel = Bevel {
        segments: 3,
        profile: 0.7,
        offset_type: BevelOffsetType::Percent,
        width: 20.0,
        limit_method: BevelLimitMethod::Weight,
        mark_seam: true,
        mark_sharp: true,
        material: 1,
        ..Bevel::default()
    };
    let evaluated = run_in(&CUBE_WEIGHTS_INPUT.build(&[]), bevel, &scene);
    assert_mesh(&evaluated.mesh, &BEVEL_WEIGHTS);
}

#[test]
fn bevel_harden_normals_match_blender() {
    let bevel = Bevel {
        segments: 2,
        harden_normals: true,
        face_strength_mode: BevelFaceStrength::Affected,
        width: 0.25,
        ..Bevel::default()
    };
    let evaluated = run(&CUBE_INPUT.build(&[]), bevel);
    assert_mesh(&evaluated.mesh, &BEVEL_HARDEN);
    assert_corner_normals(&evaluated.mesh, &BEVEL_HARDEN, BEVEL_HARDEN_NORMALS);
    assert_face_values(
        &evaluated.mesh,
        FACE_STRENGTH,
        &BEVEL_HARDEN,
        BEVEL_HARDEN_STRENGTH,
    );
}

#[test]
fn bevel_origins_point_back_to_sources() {
    let mesh = CUBE_INPUT.build(&[]);
    let evaluated = run(&mesh, Bevel::default());
    let result = &evaluated.mesh;
    assert_eq!(evaluated.origins.points.len(), result.point_count());
    assert_eq!(evaluated.origins.faces.len(), result.face_count());
    for (point, origin) in evaluated.origins.points.iter().enumerate() {
        if let Some(origin) = origin {
            let distance = result.positions[point].distance(mesh.positions[*origin as usize]);
            assert!(distance < 0.2, "point {point} far from {origin}");
        }
    }
    let input_normals = mesh.face_normals();
    let normals = result.face_normals();
    let mut mapped = 0;
    for (face, normal) in normals.iter().enumerate() {
        let Some(source) = input_normals
            .iter()
            .position(|input| input.abs_diff_eq(*normal, 1.0e-5))
        else {
            continue;
        };
        assert_eq!(evaluated.origins.faces[face], Some(source as u32));
        mapped += 1;
    }
    assert_eq!(mapped, 6);
}

#[test]
fn bevel_without_width_is_skipped() {
    let mesh = CUBE_INPUT.build(&[]);
    let stack = [Modifier::new(
        "Bevel",
        Bevel {
            width: 0.0,
            ..Bevel::default()
        },
    )];
    let evaluated = evaluate(&mesh, &stack, &Scene::default(), Purpose::Viewport);
    assert_eq!(evaluated.mesh.positions, mesh.positions);
    assert_eq!(
        evaluated
            .notes
            .iter()
            .map(|note| &note.kind)
            .collect::<Vec<_>>(),
        [&NoteKind::Skipped(SkipReason::NoWidth)]
    );
}
