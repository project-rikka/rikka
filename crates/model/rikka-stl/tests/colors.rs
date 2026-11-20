use rikka_model_core::glam::{Vec3, Vec4};
use rikka_model_core::{Material, Mesh, ModelBuilder, Node, Submesh, Vertices};
use rikka_stl::StlExporter;

use crate::support::{binary, read, written};

const TRIANGLE: [[f32; 3]; 4] = [[0.0, 0.0, 1.0], [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

fn colors(bytes: Vec<u8>) -> Vec<[u8; 4]> {
    let model = read(bytes, "painted.stl");
    assert!(model.materials.is_empty(), "STL has no materials");
    let mesh = &model.meshes[0];
    let mut seen: Vec<[u8; 4]> = mesh
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|triangle| mesh.vertices.colors[triangle[0] as usize])
        .collect();
    seen.dedup();
    seen
}

#[test]
fn plain_files_read_as_geometry_only() {
    let model = read(
        binary(b"plain header", &[(TRIANGLE, 0), (TRIANGLE, 0)]),
        "plain.stl",
    );

    assert!(model.materials.is_empty());
    assert!(model.meshes[0].vertices.colors.is_empty());
    assert!(
        model.meshes[0]
            .submeshes
            .iter()
            .all(|submesh| submesh.material.is_none())
    );
}

#[test]
fn viscam_colours_apply_per_facet() {
    let red = 1 << 15 | 31 << 10;
    let seen = colors(binary(b"plain header", &[(TRIANGLE, red), (TRIANGLE, 0)]));

    assert_eq!(seen, vec![[255, 0, 0, 255], [204, 204, 204, 255]]);
}

#[test]
fn magics_colours_default_to_header_colour() {
    let mut header = b"COLOR=".to_vec();
    header.extend_from_slice(&[255, 255, 255, 255]);
    let blue = 31 << 10;
    let seen = colors(binary(&header, &[(TRIANGLE, blue), (TRIANGLE, 1 << 15)]));

    assert_eq!(seen, vec![[0, 0, 255, 255], [255, 255, 255, 255]]);
}

#[test]
fn distinct_material_colours_become_facet_colours() {
    let mut builder = ModelBuilder::new();
    let green =
        builder.add_material(Material::new("green").base_color(Vec4::new(0.0, 1.0, 0.0, 1.0)));
    let white = builder.add_material(Material::new("white").base_color(Vec4::ONE));
    let mut mesh = Mesh::new("pair");
    mesh.vertices = Vertices {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        ..Vertices::default()
    };
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    for (first, material) in [(0, green), (3, white)] {
        mesh.submeshes.push(Submesh {
            first_index: first,
            index_count: 3,
            material: Some(material),
            ..Submesh::default()
        });
    }
    let mesh = builder.add_mesh(mesh);
    builder.add_node(Node::new("pair").mesh(mesh));
    let model = builder.finish().expect("a valid model");

    let seen = colors(written(&model, StlExporter::binary()));

    assert_eq!(seen, vec![[0, 255, 0, 255], [255, 255, 255, 255]]);
}
