use rikka_mesh_bevel::{BevelOptions, FaceStrength};

use crate::support::expect_operator;

fn options(width: f32, segments: u32) -> BevelOptions {
    BevelOptions {
        width,
        segments,
        ..BevelOptions::default()
    }
}

#[test]
fn harden_normals_match_blender() {
    let cases: [(&str, BevelOptions); 2] = [
        ("CUBE_HARDEN", options(0.2, 3)),
        ("CYLINDER_HARDEN", options(0.15, 2)),
    ];
    for (case, options) in &cases {
        expect_operator(
            case,
            &BevelOptions {
                harden_normals: true,
                ..options.clone()
            },
        );
    }
}

#[test]
fn face_strength_marks_faces_by_kind() {
    let cases: [(&str, FaceStrength); 2] = [
        ("CUBE_STRENGTH", FaceStrength::All),
        ("CUBE_STRENGTH_AFFECTED", FaceStrength::Affected),
    ];
    for (case, face_strength) in cases {
        expect_operator(
            case,
            &BevelOptions {
                face_strength,
                ..options(0.2, 2)
            },
        );
    }
}

#[test]
fn material_applies_to_new_faces() {
    expect_operator(
        "CUBE_MATERIAL",
        &BevelOptions {
            material: Some(1),
            ..options(0.2, 2)
        },
    );
}

#[test]
fn seams_and_sharp_edges_extend_along_bevel() {
    expect_operator(
        "CUBE_MARKS",
        &BevelOptions {
            mark_seam: true,
            mark_sharp: true,
            ..options(0.2, 3)
        },
    );
}
