use rikka_model_core::glam::{Quat, Vec3, Vec4};
use rikka_model_core::{Material, Mesh, Model, ModelBuilder, Node, Submesh, Transform, Vertices};
use rikka_stl::StlExporter;

use crate::support::{CUBES, TEAPOT, area, binary, read, triangles, written};

fn close(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, 1.0e-6),
        "{actual} is not {expected}"
    );
}

#[test]
fn teapot_matches_trimesh() {
    let model = read(TEAPOT, "teapot.stl");

    assert_eq!(model.metadata.importer, "STL");
    assert!(model.warnings.is_empty(), "{:?}", model.warnings);
    assert!(
        model
            .metadata
            .generator
            .as_deref()
            .is_some_and(|generator| generator.starts_with("Binary STL output from Blender"))
    );
    assert_eq!(model.triangle_count(), 894);
    let bounds = model.bounds();
    close(
        bounds.min,
        Vec3::new(-28.859_18, 0.870_107_4, -19.654_177) * 0.001,
    );
    close(
        bounds.max,
        Vec3::new(34.310_524, 30.351_412, 19.654_177) * 0.001,
    );
    assert!((area(&model) / 4_941.369_6e-6 - 1.0).abs() < 1.0e-4);

    let mesh = &model.meshes[0];
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] = triangle.map(|vertex| mesh.vertices.positions[vertex as usize]);
        let facing = (b - a).cross(c - a).normalize_or_zero();
        for vertex in triangle {
            let normal = mesh.vertices.normals[*vertex as usize];
            assert!(facing == Vec3::ZERO || normal.dot(facing) > 0.999);
        }
    }
}

#[test]
fn ascii_solids_become_named_nodes() {
    let model = read(CUBES, "cubes.stl");

    let names: Vec<&str> = model.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["CubeExportedFromCAD", "TranslatedCubeExportedFromCAD"]
    );
    assert_eq!(model.triangle_count(), 24);
    assert!((area(&model) / 12.0e-6 - 1.0).abs() < 1.0e-5);
    close(model.bounds().max, Vec3::new(6.0, 1.0, 0.0) * 0.001);
}

#[test]
fn binary_with_solid_header_reads_as_binary() {
    let bytes = binary(
        b"solid written by an exporter that ignores the advice",
        &[(
            [[0.0, 0.0, 1.0], [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            0,
        )],
    );

    let model = read(bytes, "careless.stl");

    assert_eq!(model.triangle_count(), 1);
    assert!(model.metadata.generator.is_none());
}

#[test]
fn triangles_round_trip() {
    let teapot = read(TEAPOT, "teapot.stl");
    let cubes = read(CUBES, "cubes.stl");

    for exporter in [StlExporter::binary(), StlExporter::text()] {
        let again = read(written(&teapot, exporter), "again.stl");
        let (before, after) = (triangles(&teapot), triangles(&again));
        assert_eq!(before.len(), after.len());
        for (expected, actual) in before.iter().zip(&after) {
            for (expected, actual) in expected.iter().zip(actual) {
                assert!(actual.abs_diff_eq(*expected, 1.0e-8));
            }
        }
    }

    let again = read(written(&cubes, StlExporter::text()), "again.stl");
    let names: Vec<&str> = again.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["CubeExportedFromCAD", "TranslatedCubeExportedFromCAD"]
    );
}

#[test]
fn mirrored_node_keeps_outward_normals() {
    let mut builder = ModelBuilder::new();
    let material = builder.add_material(Material::new("plain").base_color(Vec4::ONE));
    let mut mesh = Mesh::new("triangle");
    mesh.vertices = Vertices {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        ..Vertices::default()
    };
    mesh.indices = vec![0, 1, 2];
    mesh.submeshes.push(Submesh {
        index_count: 3,
        material: Some(material),
        ..Submesh::default()
    });
    let mesh = builder.add_mesh(mesh);
    builder.add_node(Node::new("mirrored").mesh(mesh).transform(Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::new(-1.0, 1.0, 1.0),
    }));
    let model: Model = builder.finish().expect("a valid model");

    let again = read(written(&model, StlExporter::binary()), "mirrored.stl");

    let [a, b, c] = triangles(&again)[0];
    let facing = (b - a).cross(c - a).normalize();
    assert!(facing.dot(Vec3::Z) > 0.999, "{facing}");
}
