use std::sync::Arc;

use rikka_fbx::FbxExporter;
use rikka_model_core::glam::Vec3;
use rikka_model_core::{Export, Exporter, Import, MemorySink, Mesh, MeshId, Model, Polygons, Sink};

use crate::support::{KNIGHT, registry};

const COLUMN: &[u8] = include_bytes!("../assets/polygons.fbx");
const TOLERANCE: f32 = 1e-5;

fn read(name: &str, bytes: Vec<u8>) -> Model {
    Import::named_bytes(name, bytes, registry())
        .run()
        .expect("the document is read")
}

fn exported(model: &Model, exporter: FbxExporter) -> Vec<u8> {
    let sink = Arc::new(MemorySink::new());
    let exported = Export::with_exporter(model, Arc::new(exporter))
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes("written.fbx")
        .expect("the model is written");
    assert!(exported.warnings.is_empty(), "{:?}", exported.warnings);
    exported.bytes
}

fn mesh_named<'a>(model: &'a Model, name: &str) -> (MeshId, &'a Mesh) {
    let index = model
        .meshes
        .iter()
        .position(|mesh| mesh.name == name)
        .expect("the mesh is present");
    (MeshId(index as u32), &model.meshes[index])
}

fn polygons(mesh: &Mesh) -> &Polygons {
    mesh.polygons.as_ref().expect("the mesh keeps its polygons")
}

fn sizes(polygons: &Polygons) -> Vec<usize> {
    (0..polygons.len())
        .map(|polygon| polygons.polygon(polygon).len())
        .collect()
}

fn polygon_material(model: &Model, mesh: &Mesh, polygon: usize) -> Option<String> {
    let submesh = polygons(mesh).submeshes[polygon] as usize;
    mesh.submeshes[submesh]
        .material
        .map(|material| model.material(material).name.clone())
}

fn influences(model: &Model, mesh: MeshId, vertex: u32) -> Vec<(String, f32)> {
    let Some(skin) = model
        .nodes
        .iter()
        .find(|node| node.mesh == Some(mesh))
        .and_then(|node| node.skin)
    else {
        return Vec::new();
    };
    let joints = &model.skin(skin).joints;
    let mut named: Vec<(String, f32)> = model
        .mesh(mesh)
        .vertices
        .influences(vertex as usize)
        .into_iter()
        .flatten()
        .filter(|&(_, weight)| weight > 0.0)
        .map(|(slot, weight)| (model.node(joints[usize::from(slot)]).name.clone(), weight))
        .collect();
    named.sort_by(|one, two| one.0.cmp(&two.0));
    named
}

fn displacement(mesh: &Mesh, target: usize, vertex: u32) -> Vec3 {
    let target = &mesh.morph_targets[target];
    target
        .vertices
        .iter()
        .position(|&moved| moved == vertex)
        .map_or(Vec3::ZERO, |slot| target.positions[slot])
}

fn assert_close(one: Vec3, two: Vec3, what: &str) {
    assert!(
        one.abs_diff_eq(two, TOLERANCE),
        "{what}: {one} against {two}"
    );
}

fn assert_same_polygons(before: &Model, after: &Model, name: &str) {
    let ((one_id, one), (two_id, two)) = (mesh_named(before, name), mesh_named(after, name));
    let (first, second) = (polygons(one), polygons(two));
    assert_eq!(first.offsets, second.offsets, "{name}: polygon sizes");
    assert_eq!(
        first.point_count(),
        second.point_count(),
        "{name}: control points"
    );
    assert_eq!(
        one.vertices.len(),
        two.vertices.len(),
        "{name}: vertex count"
    );
    assert_eq!(
        one.morph_targets.len(),
        two.morph_targets.len(),
        "{name}: blend shapes"
    );

    for polygon in 0..first.len() {
        assert_eq!(
            polygon_material(before, one, polygon),
            polygon_material(after, two, polygon),
            "{name}: material of polygon {polygon}"
        );
    }
    for (corner, (&a, &b)) in first
        .corner_vertices
        .iter()
        .zip(&second.corner_vertices)
        .enumerate()
    {
        let what = format!("{name}: corner {corner}");
        assert_eq!(
            first.vertex_points[a as usize], second.vertex_points[b as usize],
            "{what} point"
        );
        let (a_index, b_index) = (a as usize, b as usize);
        assert_close(
            one.vertices.positions[a_index],
            two.vertices.positions[b_index],
            &what,
        );
        assert_close(
            one.vertices.normals[a_index],
            two.vertices.normals[b_index],
            &what,
        );
        if !one.vertices.uvs.is_empty() {
            assert!(
                one.vertices.uvs[a_index].abs_diff_eq(two.vertices.uvs[b_index], TOLERANCE),
                "{what} uv"
            );
        }
        assert_eq!(
            one.vertices.colors.get(a_index),
            two.vertices.colors.get(b_index),
            "{what} colour"
        );
        let (weights, reread) = (influences(before, one_id, a), influences(after, two_id, b));
        assert_eq!(weights.len(), reread.len(), "{what} influences");
        for ((joint, weight), (reread_joint, reread_weight)) in weights.iter().zip(&reread) {
            assert_eq!(joint, reread_joint, "{what} joint");
            assert!((weight - reread_weight).abs() < TOLERANCE, "{what} weight");
        }
        for target in 0..one.morph_targets.len() {
            assert_close(
                displacement(one, target, a),
                displacement(two, target, b),
                &format!("{what} shape {target}"),
            );
        }
    }
}

#[test]
fn blender_quads_and_ngons_read_as_polygons() {
    let model = read("polygons.fbx", COLUMN.to_vec());
    let (id, mesh) = mesh_named(&model, "Column");
    let polygons = polygons(mesh);

    let mut sides = sizes(polygons);
    sides.sort_unstable();
    assert_eq!(sides, [4, 4, 4, 4, 4, 4, 4, 4, 8, 8]);
    assert_eq!(polygons.point_count(), 16);
    assert_eq!(mesh.triangle_count(), 8 * 2 + 2 * 6);
    assert_eq!(mesh.morph_targets.len(), 1);
    let weights = &model
        .nodes
        .iter()
        .find(|node| node.mesh == Some(id))
        .expect("the node that draws the column")
        .morph_weights;
    assert!(
        weights.len() == 1 && (weights[0] - 0.25).abs() < 1e-4,
        "{weights:?}"
    );

    for polygon in 0..polygons.len() {
        let expected = match polygons.polygon(polygon).len() {
            8 => "Cap",
            _ => "Side",
        };
        assert_eq!(
            polygon_material(&model, mesh, polygon).as_deref(),
            Some(expected)
        );
    }
    let mut lower: Vec<(u32, f32)> = (0..mesh.vertices.len() as u32)
        .map(|vertex| {
            let weight = influences(&model, id, vertex)
                .into_iter()
                .find(|(joint, _)| joint == "Lower")
                .map_or(0.0, |(_, weight)| weight);
            (polygons.vertex_points[vertex as usize], weight)
        })
        .collect();
    lower.sort_by_key(|&(point, _)| point);
    lower.dedup_by_key(|&mut (point, _)| point);
    let bottom = lower
        .iter()
        .filter(|&&(_, weight)| (weight - 1.0).abs() < 1e-4)
        .count();
    let top = lower
        .iter()
        .filter(|&&(_, weight)| (weight - 0.25).abs() < 1e-4)
        .count();
    assert_eq!((bottom, top), (8, 8), "{lower:?}");
}

#[test]
fn polygons_round_trip_through_binary_and_text() {
    let column = read("polygons.fbx", COLUMN.to_vec());
    let knight = read("knight.fbx", KNIGHT.to_vec());
    for exporter in [FbxExporter::binary(), FbxExporter::text()] {
        let name = exporter.info().name;
        let again = read("written.fbx", exported(&column, exporter));
        assert_same_polygons(&column, &again, "Column");
        let twice = read("written.fbx", exported(&again, exporter));
        assert_same_polygons(&again, &twice, "Column");

        let knight_again = read("written.fbx", exported(&knight, exporter));
        for mesh in &knight.meshes {
            assert!(mesh.polygons.is_some(), "{name}: {} polygons", mesh.name);
            assert_same_polygons(&knight, &knight_again, &mesh.name);
        }
    }
}

#[test]
fn written_polygons_share_control_points() {
    let column = read("polygons.fbx", COLUMN.to_vec());
    let text = String::from_utf8(exported(&column, FbxExporter::text())).expect("text");

    for record in [
        "Vertices: *48 {",
        "PolygonVertexIndex: *48 {",
        "Edges: *24 {",
        "Indexes: *16 {",
        "Smoothing: *24 {",
        "MappingInformationType: \"ByEdge\"",
        "MappingInformationType: \"ByPolygon\"",
        "Materials: *10 {",
    ] {
        assert!(text.contains(record), "{record}");
    }
    let indices = text
        .split("PolygonVertexIndex: *48 {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("the polygon indices");
    let ends = indices.matches('-').count();
    assert_eq!(ends, 10, "{indices}");
}

#[test]
fn unused_and_repeated_control_points_round_trip() {
    let text = "; FBX 7.4.0 project file\n\
        FBXHeaderExtension:  {\n\
        \tFBXHeaderVersion: 1003\n\
        \tFBXVersion: 7400\n\
        }\n\
        Objects:  {\n\
        \tGeometry: 10, \"Geometry::plate\", \"Mesh\" {\n\
        \t\tVertices: *18 {\n\
        \t\t\ta: 9,9,9,0,0,0,1,0,0,1,1,0,0,1,0,2,0,0\n\
        \t\t}\n\
        \t\tPolygonVertexIndex: *8 {\n\
        \t\t\ta: 1,2,3,-5,2,5,5,-4\n\
        \t\t}\n\
        \t\tLayerElementMaterial: 0 {\n\
        \t\t\tMappingInformationType: \"ByPolygon\"\n\
        \t\t\tReferenceInformationType: \"IndexToDirect\"\n\
        \t\t\tMaterials: *2 {\n\
        \t\t\t\ta: 0,-1\n\
        \t\t\t}\n\
        \t\t}\n\
        \t}\n\
        \tModel: 20, \"Model::plate\", \"Mesh\" {\n\
        \t}\n\
        \tMaterial: 30, \"Material::paint\", \"\" {\n\
        \t}\n\
        }\n\
        Connections:  {\n\
        \tC: \"OO\",20,0\n\
        \tC: \"OO\",10,20\n\
        \tC: \"OO\",30,20\n\
        }\n";
    let model = read("plate.fbx", text.as_bytes().to_vec());
    let (_, mesh) = mesh_named(&model, "plate");
    let plate = polygons(mesh);

    assert_eq!(sizes(plate), [4, 4]);
    let points: Vec<u32> = plate
        .corner_vertices
        .iter()
        .map(|&vertex| plate.vertex_points[vertex as usize])
        .collect();
    assert_eq!(points, [1, 2, 3, 4, 2, 5, 5, 3]);
    assert_eq!(polygon_material(&model, mesh, 0).as_deref(), Some("paint"));
    assert_eq!(polygon_material(&model, mesh, 1), None);
    assert_eq!(plate.point_count(), 6);

    let loose = |mesh: &Mesh| {
        let polygons = polygons(mesh);
        let vertex = polygons
            .vertex_points
            .iter()
            .position(|&point| point == 0)
            .expect("the unused control point stays a loose vertex");
        assert!(!polygons.corner_vertices.contains(&(vertex as u32)));
        mesh.vertices.positions[vertex]
    };
    for exporter in [FbxExporter::binary(), FbxExporter::text()] {
        let again = read("written.fbx", exported(&model, exporter));
        assert_same_polygons(&model, &again, "plate");
        assert_close(
            loose(mesh),
            loose(mesh_named(&again, "plate").1),
            "loose point",
        );
    }
}
