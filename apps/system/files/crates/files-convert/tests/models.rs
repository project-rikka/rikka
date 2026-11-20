use std::collections::BTreeSet;
use std::path::Path;

use files_convert::{ModelFormat, Target};
use rikka_model::glam::{Vec2, Vec3};
use rikka_model::{
    Export, ExportOptions, Image, Material, MaterialTextures, Mesh, ModelBuilder, Node, Submesh,
    Texture, Vertices, import, registry,
};

use crate::support::{names, succeeded};

fn textured_model(path: &Path) {
    let picture = rikka_image::Image::filled(
        2,
        2,
        rikka_image::ColorType::Rgb8,
        rikka_types::Color::WHITE,
    )
    .expect("encode texture")
    .encode(rikka_image::Format::Png)
    .expect("png bytes");
    let mut builder = ModelBuilder::new().importer("test");
    let image = builder.add_image(Image::new("skin.png", picture));
    let texture = builder.add_texture(Texture::new(image));
    let skin = builder.add_material(Material::new("skin").textures(MaterialTextures {
        base_color: Some(texture),
        ..MaterialTextures::default()
    }));
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![Vec2::ZERO, Vec2::X, Vec2::Y],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2],
        submeshes: vec![Submesh {
            first_index: 0,
            index_count: 3,
            material: Some(skin),
            ..Submesh::default()
        }],
        ..Mesh::new("body")
    });
    builder.add_node(Node::new("textured_model").mesh(mesh));
    let model = builder.finish().expect("build model");
    Export::new(&model, registry())
        .options(ExportOptions::new().embed_resources(true))
        .to_file(path)
        .expect("export source model");
}

#[test]
fn model_converts_to_file_or_folder() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("textured_model.glb");
    textured_model(&source);

    for (format, alone) in [
        (ModelFormat::Gltf, false),
        (ModelFormat::Fbx, true),
        (ModelFormat::Pmx, false),
    ] {
        let before: BTreeSet<String> = names(folder.path()).into_iter().collect();
        succeeded(&[&source], Target::Model(format), "");
        let after: BTreeSet<String> = names(folder.path()).into_iter().collect();
        let created: Vec<&String> = after.difference(&before).collect();
        assert_eq!(created.len(), 1, "{format:?} created {created:?}");
        let created = folder.path().join(created[0]);
        let main = if alone {
            assert!(created.is_file(), "{format:?} is one file");
            created
        } else {
            assert!(created.is_dir(), "{format:?} brings its resources");
            created.join(format!("textured_model.{}", format.extension()))
        };
        let model = import(&main).unwrap_or_else(|error| panic!("{format:?}: {error}"));
        assert_eq!(model.meshes.len(), 1, "{format:?}");
    }
}
