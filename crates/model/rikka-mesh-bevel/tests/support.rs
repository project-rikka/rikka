use glam::{Vec2, Vec3};
use rikka_mesh_bevel::{BevelInput, BevelOptions, bevel, bevel_selected};
use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, Flags, VertId};
use rikka_polymesh::Domain;

use crate::oracle::{Case, Input, Output, case, input, summary};

const POSITION_TOLERANCE: f32 = 2.0e-4;
const UV_TOLERANCE: f32 = 2.0e-4;
const NORMAL_TOLERANCE: f32 = 3.0e-3;
const UV_MAP: &str = "UVMap";
const GROUP: &str = "Group";
const STRENGTH: &str = "__mod_weightednormals_faceweight";

pub(crate) fn checked(mesh: &EditMesh) {
    if let Err(error) = mesh.validate() {
        panic!("invalid edit mesh: {error}");
    }
}

pub(crate) fn build(input: &Input) -> EditMesh {
    let mut mesh = EditMesh::new();
    let verts: Vec<VertId> = input.verts.iter().map(|&co| mesh.add_vert(co)).collect();
    for &[a, b] in &input.edges {
        mesh.add_edge(verts[a as usize], verts[b as usize])
            .expect("fixture edge");
    }
    for face in &input.faces {
        let corners: Vec<VertId> = face.iter().map(|&index| verts[index as usize]).collect();
        mesh.new_face(&corners)
            .existing_edges()
            .create()
            .expect("fixture face");
    }
    if !input.uvs.is_empty() {
        mesh.add_uv_map(UV_MAP).expect("fixture uv map");
        let loops: Vec<_> = mesh
            .faces()
            .flat_map(|face| mesh.face_loops(face).collect::<Vec<_>>())
            .collect();
        let uvs = mesh.loop_uvs_mut(UV_MAP).expect("fixture uv map");
        for (lp, &uv) in loops.into_iter().zip(&input.uvs) {
            uvs[lp.index()] = uv;
        }
    }
    for &edge in &input.seams {
        mesh.set_edge_seam(EdgeId::new(edge), true);
    }
    for &edge in &input.sharp {
        mesh.set_edge_sharp(EdgeId::new(edge), true);
    }
    for &face in &input.smooth {
        mesh.set_face_smooth(FaceId::new(face), true);
    }
    if !input.edge_weights.is_empty() {
        let weights = mesh.edge_bevel_weights_mut();
        for &(edge, weight) in &input.edge_weights {
            weights[edge as usize] = weight;
        }
    }
    if !input.vert_weights.is_empty() {
        let weights = mesh.vert_bevel_weights_mut();
        for &(vert, weight) in &input.vert_weights {
            weights[vert as usize] = weight;
        }
    }
    if !input.group.is_empty() {
        let group = mesh.add_vertex_group(GROUP);
        for &(vert, weight) in &input.group {
            mesh.set_vert_weight(VertId::new(vert), group, weight);
        }
    }
    mesh.update_normals();
    checked(&mesh);
    mesh
}

pub(crate) fn build_case(name: &str) -> EditMesh {
    build(input(&case(name).input))
}

fn operate(
    name: &str,
    input: &Input,
    selection: (&[u32], &[u32]),
    options: &BevelOptions,
) -> EditMesh {
    let mut mesh = build(input);
    for &vert in selection.0 {
        mesh.set_vert_flag(VertId::new(vert), Flags::SELECT, true);
    }
    for &edge in selection.1 {
        mesh.set_edge_flag(EdgeId::new(edge), Flags::SELECT, true);
    }
    bevel_selected(&mut mesh, options).unwrap_or_else(|error| panic!("{name}: {error}"));
    checked(&mesh);
    mesh
}

pub(crate) fn expect_operator(name: &str, options: &BevelOptions) -> EditMesh {
    let case = case(name);
    let mesh = operate(
        name,
        input(&case.input),
        (&case.verts, &case.edges),
        options,
    );
    compare(case, &mesh);
    mesh
}

pub(crate) fn expect_input(
    name: &str,
    make: impl FnOnce(&EditMesh) -> BevelInput,
    options: &BevelOptions,
) -> EditMesh {
    let case = case(name);
    let mut mesh = build(input(&case.input));
    let selection = make(&mesh);
    bevel(&mut mesh, &selection, options).unwrap_or_else(|error| panic!("{name}: {error}"));
    checked(&mesh);
    compare(case, &mesh);
    mesh
}

pub(crate) fn expect_summary(name: &str, options: &BevelOptions) {
    let summary = summary(name);
    let mesh = operate(
        name,
        input(&summary.input),
        (&summary.verts, &summary.edges),
        options,
    );
    let counts = [
        mesh.vert_count() as u32,
        mesh.edge_count() as u32,
        mesh.face_count() as u32,
    ];
    assert_eq!(counts, summary.counts, "{name}: element counts");
    let mut sizes: Vec<(u32, u32)> = Vec::new();
    for face in mesh.faces() {
        let size = mesh.face_len(face) as u32;
        match sizes.iter_mut().find(|(len, _)| *len == size) {
            Some((_, count)) => *count += 1,
            None => sizes.push((size, 1)),
        }
    }
    sizes.sort_unstable();
    assert_eq!(sizes, summary.face_sizes, "{name}: face sizes");
    let position_sum: Vec3 = mesh.verts().map(|vert| mesh.position(vert)).sum();
    assert!(
        position_sum.distance(summary.position_sum)
            <= POSITION_TOLERANCE * summary.counts[0] as f32,
        "{name}: position sum {position_sum} vs blender {}",
        summary.position_sum
    );
    let edge_length: f32 = mesh.edges().map(|edge| mesh.edge_length(edge)).sum();
    assert!(
        (edge_length - summary.edge_length).abs() <= POSITION_TOLERANCE * summary.counts[1] as f32,
        "{name}: edge length {edge_length} vs blender {}",
        summary.edge_length
    );
}

#[derive(Clone, Copy)]
struct Corner {
    position: Vec3,
    uv: Option<Vec2>,
    normal: Option<Vec3>,
}

struct ActualFace {
    face: FaceId,
    corners: Vec<Corner>,
}

fn actual_faces(mesh: &EditMesh) -> Vec<ActualFace> {
    let uvs = mesh.loop_uvs(UV_MAP);
    let normals = mesh.custom_normals();
    mesh.faces()
        .map(|face| ActualFace {
            face,
            corners: mesh
                .face_loops(face)
                .map(|lp| Corner {
                    position: mesh.position(mesh.loop_vert(lp)),
                    uv: uvs.map(|uvs| uvs[lp.index()]),
                    normal: normals.map(|normals| normals[lp.index()].normalize_or_zero()),
                })
                .collect(),
        })
        .collect()
}

fn expected_faces(output: &Output) -> Vec<Vec<Corner>> {
    output
        .faces
        .iter()
        .enumerate()
        .map(|(index, face)| {
            face.iter()
                .enumerate()
                .map(|(corner, &position)| Corner {
                    position,
                    uv: output.uvs.get(index).map(|uvs| uvs[corner]),
                    normal: output.normals.get(index).map(|normals| normals[corner]),
                })
                .collect()
        })
        .collect()
}

fn corner_matches(expected: &Corner, actual: &Corner) -> bool {
    if expected.position.distance(actual.position) > POSITION_TOLERANCE {
        return false;
    }
    if let (Some(expected), Some(actual)) = (expected.uv, actual.uv)
        && expected.distance(actual) > UV_TOLERANCE
    {
        return false;
    }
    if let Some(expected) = expected.normal {
        let Some(actual) = actual.normal else {
            return false;
        };
        if expected.distance(actual) > NORMAL_TOLERANCE {
            return false;
        }
    }
    true
}

fn face_matches(expected: &[Corner], actual: &[Corner]) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let count = expected.len();
    (0..count)
        .any(|shift| (0..count).all(|i| corner_matches(&expected[i], &actual[(i + shift) % count])))
}

fn positions_match(expected: &[Corner], actual: &[Corner]) -> bool {
    let strip = |corners: &[Corner]| -> Vec<Corner> {
        corners
            .iter()
            .map(|corner| Corner {
                uv: None,
                normal: None,
                ..*corner
            })
            .collect()
    };
    face_matches(&strip(expected), &strip(actual))
}

fn describe(corners: &[Corner]) -> String {
    corners
        .iter()
        .map(|corner| {
            let mut text = format!(
                "({:.5}, {:.5}, {:.5})",
                corner.position.x, corner.position.y, corner.position.z
            );
            if let Some(uv) = corner.uv {
                text += &format!(" uv({:.5}, {:.5})", uv.x, uv.y);
            }
            if let Some(normal) = corner.normal {
                text += &format!(" n({:.4}, {:.4}, {:.4})", normal.x, normal.y, normal.z);
            }
            text
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn edge_set_matches(
    mesh: &EditMesh,
    expected: &[[Vec3; 2]],
    marked: impl Fn(EdgeId) -> bool,
) -> bool {
    let actual: Vec<[Vec3; 2]> = mesh
        .edges()
        .filter(|&edge| marked(edge))
        .map(|edge| mesh.edge_verts(edge).map(|vert| mesh.position(vert)))
        .collect();
    actual.len() == expected.len()
        && expected.iter().all(|&[a, b]| {
            actual.iter().any(|&[c, d]| {
                (a.distance(c) <= POSITION_TOLERANCE && b.distance(d) <= POSITION_TOLERANCE)
                    || (a.distance(d) <= POSITION_TOLERANCE && b.distance(c) <= POSITION_TOLERANCE)
            })
        })
}

fn compare(case: &Case, mesh: &EditMesh) {
    let output = &case.output;
    let name = &case.name;
    let counts = [
        mesh.vert_count() as u32,
        mesh.edge_count() as u32,
        mesh.face_count() as u32,
    ];
    let actual = actual_faces(mesh);
    let expected = expected_faces(output);
    let mut used = vec![false; actual.len()];
    let mut failures = Vec::new();
    for (index, face) in expected.iter().enumerate() {
        let found = (0..actual.len()).find(|&i| !used[i] && face_matches(face, &actual[i].corners));
        let Some(i) = found else {
            let candidate = actual
                .iter()
                .enumerate()
                .find(|(i, other)| !used[*i] && positions_match(face, &other.corners))
                .map(|(_, other)| describe(&other.corners));
            failures.push(format!(
                "expected {}\n  same positions: {}",
                describe(face),
                candidate.unwrap_or_else(|| "none".to_owned())
            ));
            continue;
        };
        used[i] = true;
        let actual_face = actual[i].face;
        if let Some(&material) = output.materials.get(index) {
            assert_eq!(
                mesh.face_material(actual_face),
                material,
                "{name}: material of face {}",
                describe(face)
            );
        }
        if let Some(&strength) = output.strength.get(index) {
            let values = mesh
                .attribute::<i32>(STRENGTH, Domain::Face)
                .unwrap_or_else(|| panic!("{name}: missing face strength"));
            assert_eq!(
                values[actual_face.index()],
                strength,
                "{name}: face strength"
            );
        }
    }
    if !failures.is_empty() || counts != output.counts {
        let unmatched: Vec<String> = actual
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .map(|(_, face)| describe(&face.corners))
            .collect();
        panic!(
            "{name}: counts {counts:?} vs blender {:?}; {} faces differ:\n{}\nunmatched actual faces:\n{}",
            output.counts,
            failures.len(),
            failures.join("\n"),
            unmatched.join("\n")
        );
    }
    assert!(
        edge_set_matches(mesh, &output.seams, |edge| mesh.is_edge_seam(edge)),
        "{name}: seam edges differ"
    );
    assert!(
        edge_set_matches(mesh, &output.sharp, |edge| mesh.is_edge_sharp(edge)),
        "{name}: sharp edges differ"
    );
    if let Some(selected) = output.selected {
        let actual = [
            mesh.verts()
                .filter(|&vert| mesh.is_vert_selected(vert))
                .count() as u32,
            mesh.edges()
                .filter(|&edge| mesh.is_edge_selected(edge))
                .count() as u32,
            mesh.faces()
                .filter(|&face| mesh.is_face_selected(face))
                .count() as u32,
        ];
        assert_eq!(actual, selected, "{name}: selection counts");
    }
}
