use std::fs;
use std::sync::Arc;

use rikka_model_core::glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use rikka_model_core::{
    Animation, Camera, Channel, Curve, Export, ExportOptions, Image, Import, Interpolation, Light,
    LightKind, Material, MaterialTextures, MemorySink, Mesh, Model, ModelBuilder, MorphTarget,
    Node, Projection, Sampler, Shading, Sink, Skin, Submesh, Texture, Track, Transform, Vertices,
    Weights,
};

use crate::support::registry;

const FOX: &[u8] = include_bytes!("../assets/fox.glb");

fn fox() -> Model {
    Import::named_bytes("fox.glb", FOX.to_vec(), registry())
        .run()
        .expect("the sample fox is read")
}

fn written(model: &Model, name: &str, options: ExportOptions) -> Model {
    let sink = Arc::new(MemorySink::new());
    let exported = Export::new(model, registry())
        .options(options)
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes(name)
        .expect("the model is written");
    assert!(exported.warnings.is_empty(), "{:?}", exported.warnings);

    let mut resolver = rikka_model_core::MemoryResolver::new();
    for name in sink.names() {
        resolver.insert(&name, sink.get(&name).expect("what was written"));
    }
    Import::named_bytes(name, exported.bytes, registry())
        .resolver(Arc::new(resolver))
        .run()
        .expect("the written model is read again")
}

#[test]
fn sample_fox_reads() {
    let model = fox();

    assert_eq!(model.metadata.importer, "glTF");
    assert!(model.warnings.is_empty(), "{:?}", model.warnings);
    assert_eq!(model.nodes.len(), 26);
    assert_eq!(model.meshes.len(), 1);
    assert_eq!(model.materials.len(), 1);
    assert_eq!(model.textures.len(), 1);

    let mesh = &model.meshes[0];
    assert_eq!(mesh.vertices.len(), 1728);
    assert_eq!(mesh.triangle_count(), 576);
    assert_eq!(mesh.vertices.uvs.len(), 1728);
    assert!(mesh.vertices.is_skinned());
    assert_eq!(mesh.submeshes[0].index_count as usize, mesh.indices.len());

    let skin = &model.skins[0];
    assert_eq!(skin.joint_count(), 24);
    assert_eq!(skin.inverse_bind.len(), 24);

    let names: Vec<&str> = model
        .animations
        .iter()
        .map(|animation| animation.name.as_str())
        .collect();
    assert_eq!(names, vec!["Survey", "Walk", "Run"]);
    assert!(
        (model
            .animation(model.find_animation("Walk").expect("a walk"))
            .duration()
            - 0.708)
            .abs()
            < 0.01
    );

    assert!(model.images[0].bytes.starts_with(b"\x89PNG"));
    assert!(model.bounds().max.y > 1.0);
}

#[test]
fn fox_round_trips_through_glb() {
    let model = fox();
    let again = written(&model, "fox.glb", ExportOptions::new());

    assert_eq!(again.nodes.len(), model.nodes.len());
    assert_eq!(again.vertex_count(), model.vertex_count());
    assert_eq!(again.triangle_count(), model.triangle_count());
    assert_eq!(again.animations.len(), model.animations.len());
    assert_eq!(again.skins[0].joint_count(), model.skins[0].joint_count());
    assert_eq!(again.images[0].bytes, model.images[0].bytes);
    assert_eq!(
        again.meshes[0].vertices.positions,
        model.meshes[0].vertices.positions
    );
    assert_eq!(again.meshes[0].indices, model.meshes[0].indices);
    for (before, after) in model.animations.iter().zip(&again.animations) {
        assert_eq!(before.name, after.name);
        assert_eq!(before.channels.len(), after.channels.len());
        assert!((before.duration() - after.duration()).abs() < 1e-6);
    }
}

#[test]
fn external_resources_round_trip() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("fox.gltf");
    let model = fox();

    let warnings = Export::new(&model, registry())
        .options(ExportOptions::new().embed_resources(false))
        .to_file(&path)
        .expect("the fox is written as a document and its files");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(folder.path().join("fox.bin").exists());

    let document = fs::read_to_string(&path).expect("the document");
    assert!(document.contains("\"uri\": \"fox.bin\""), "{document:.200}");

    let again = Import::open(&path, registry())
        .run()
        .expect("the written fox is read");
    assert_eq!(again.images[0].bytes, model.images[0].bytes);
    assert_eq!(again.vertex_count(), model.vertex_count());
}

#[test]
fn embedded_document_is_self_contained() {
    let model = fox();
    let exported = Export::new(&model, registry())
        .to_bytes("fox.gltf")
        .expect("the fox is written as one document");
    let document = String::from_utf8(exported.bytes.clone()).expect("a text document");
    assert!(document.contains("data:application/octet-stream;base64,"));
    assert!(document.contains("data:image/png;base64,"));

    let again = Import::named_bytes("fox.gltf", exported.bytes, registry())
        .run()
        .expect("a document that needs nothing beside it");
    assert_eq!(again.vertex_count(), model.vertex_count());
    assert_eq!(again.images[0].bytes, model.images[0].bytes);
}

#[test]
fn model_round_trips_losslessly() {
    let model = everything();
    let again = written(&model, "everything.glb", ExportOptions::new());

    assert_eq!(again.nodes.len(), model.nodes.len());
    assert_eq!(again.find_node("bone"), model.find_node("bone"));
    assert_eq!(again.roots, model.roots);

    let mesh = &again.meshes[0];
    assert_eq!(mesh.name, "body");
    assert_eq!(mesh.submeshes.len(), 2);
    assert_eq!(
        mesh.submeshes[1].material,
        model.meshes[0].submeshes[1].material
    );
    assert_eq!(mesh.vertices.colors, model.meshes[0].vertices.colors);
    assert_eq!(mesh.vertices.joints, model.meshes[0].vertices.joints);
    assert_eq!(mesh.vertices.uvs, model.meshes[0].vertices.uvs);
    assert_eq!(mesh.vertices.tangents, model.meshes[0].vertices.tangents);

    assert_eq!(mesh.morph_targets.len(), 1);
    let target = &mesh.morph_targets[0];
    assert_eq!(target.name, "smile");
    assert_eq!(target.vertices, vec![1]);
    assert_eq!(target.positions, vec![Vec3::Y]);
    assert_eq!(again.node(again.roots[0]).morph_weights, vec![0.5]);

    let material = &again.materials[0];
    assert_eq!(material.name, "skin");
    assert_eq!(material.base_color, Vec4::new(0.5, 0.25, 0.125, 1.0));
    assert_eq!(
        material.alpha,
        rikka_model_core::Alpha::Mask { cutoff: 0.25 }
    );
    assert!(material.double_sided);
    assert_eq!(material.shading, model.materials[0].shading);
    assert_eq!(again.textures[0].sampler, model.textures[0].sampler);
    assert_eq!(
        again.materials[0].textures.base_color,
        model.materials[0].textures.base_color
    );

    let camera = &again.cameras[0];
    assert_eq!(camera.name, "eye");
    assert_eq!(
        camera.projection,
        Projection::Orthographic {
            width: 4.0,
            height: 2.0,
            near: 0.1,
            far: 100.0,
        }
    );

    let light = &again.lights[0];
    assert_eq!(light.name, "lamp");
    assert!(matches!(light.kind, LightKind::Spot { range, .. } if range == 12.0));
    assert_eq!(light.intensity, 3.0);

    assert_eq!(again.skins[0].joints, model.skins[0].joints);
    assert_eq!(again.skins[0].inverse_bind, model.skins[0].inverse_bind);
    assert_eq!(again.skins[0].skeleton, model.skins[0].skeleton);

    let animation = &again.animations[0];
    assert_eq!(animation.name, "wave");
    assert_eq!(animation.channels.len(), 4);
    assert!((animation.duration() - 2.0).abs() < 1e-6);
    let Track::Rotation(curve) = &animation.channels[1].track else {
        panic!("the second channel turns its node");
    };
    assert_eq!(curve.interpolation, Interpolation::Step);
    let Track::Weights(weights) = &animation.channels[3].track else {
        panic!("the fourth channel drives morph weights");
    };
    assert_eq!(weights.count, 1);
    assert_eq!(weights.interpolation, Interpolation::CubicSpline);
}

#[test]
fn model_reads_sibling_files() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("art/fox.gltf");
    fs::create_dir_all(folder.path().join("art")).expect("a folder for the model");
    Export::new(&fox(), registry())
        .options(ExportOptions::new().embed_resources(false))
        .to_file(&path)
        .expect("a written fox");

    let model = Import::open(&path, registry())
        .run()
        .expect("a fox beside its files");
    assert!(model.warnings.is_empty(), "{:?}", model.warnings);
    assert_eq!(model.images.len(), 1);
}

fn everything() -> Model {
    let mut builder = ModelBuilder::new().importer("test");
    let image = builder.add_image(Image::new("skin.png", png()));
    let texture = builder.add_texture(Texture::new(image).sampler(Sampler::new(
        rikka_model_core::Filter::Nearest,
        rikka_model_core::Wrap::Clamp,
    )));
    let skin_material = builder.add_material(
        Material::new("skin")
            .base_color(Vec4::new(0.5, 0.25, 0.125, 1.0))
            .alpha(rikka_model_core::Alpha::Mask { cutoff: 0.25 })
            .double_sided(true)
            .shading(Shading::Pbr(rikka_model_core::Pbr {
                metallic: 0.25,
                roughness: 0.75,
                normal_scale: 1.0,
                occlusion_strength: 1.0,
                specular: rikka_model_core::Specular {
                    weight: 0.5,
                    color: Vec3::new(1.0, 0.5, 0.25),
                    ior: 1.33,
                },
                anisotropy: rikka_model_core::Anisotropy {
                    strength: 0.5,
                    rotation: 0.25,
                },
                coat: rikka_model_core::Coat {
                    weight: 0.75,
                    roughness: 0.125,
                    darkening: 0.0,
                    ..rikka_model_core::Coat::default()
                },
                sheen: rikka_model_core::Sheen {
                    weight: 1.0,
                    color: Vec3::new(0.25, 0.5, 1.0),
                    roughness: 0.375,
                },
                transmission: rikka_model_core::Transmission {
                    weight: 0.5,
                    thickness: 0.25,
                    attenuation_color: Vec3::new(0.5, 0.75, 1.0),
                    attenuation_distance: 2.0,
                    dispersion: 0.125,
                },
                iridescence: rikka_model_core::Iridescence {
                    weight: 0.5,
                    ior: 1.5,
                    thickness_min: 200.0,
                    thickness_max: 600.0,
                },
                ..rikka_model_core::Pbr::default()
            }))
            .textures(MaterialTextures {
                base_color: Some(texture),
                ..MaterialTextures::default()
            }),
    );
    let trim = builder.add_material(Material::new("trim").emissive(Vec3::new(2.0, 0.0, 0.0)));

    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::ONE],
            normals: vec![Vec3::Z; 4],
            tangents: vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: vec![Vec2::ZERO, Vec2::X, Vec2::Y, Vec2::ONE],
            colors: vec![[255, 128, 0, 255]; 4],
            joints: vec![[0, 0, 0, 0]; 4],
            weights: vec![Vec4::X; 4],
        },
        indices: vec![0, 1, 2, 1, 3, 2],
        submeshes: vec![
            Submesh {
                name: String::new(),
                first_index: 0,
                index_count: 3,
                material: Some(skin_material),
                ..Submesh::default()
            },
            Submesh {
                name: String::new(),
                first_index: 3,
                index_count: 3,
                material: Some(trim),
                ..Submesh::default()
            },
        ],
        morph_targets: vec![MorphTarget {
            name: "smile".into(),
            vertices: vec![1],
            positions: vec![Vec3::Y],
            normals: Vec::new(),
        }],
        ..Mesh::new("body")
    });

    let camera = builder.add_camera(Camera::new("eye").projection(Projection::Orthographic {
        width: 4.0,
        height: 2.0,
        near: 0.1,
        far: 100.0,
    }));
    let light = builder.add_light(
        Light::new(
            "lamp",
            LightKind::Spot {
                range: 12.0,
                inner_angle: 0.2,
                outer_angle: 0.5,
            },
        )
        .intensity(3.0),
    );

    let root = builder.add_node(
        Node::new("body")
            .mesh(mesh)
            .morph_weights(vec![0.5])
            .transform(Transform::from_translation(Vec3::new(0.0, 1.0, 0.0))),
    );
    let bone = builder.add_node(Node::new("bone"));
    let eye = builder.add_node(Node::new("eye").camera(camera).light(light));
    builder.attach(root, bone);
    builder.attach(root, eye);

    let skin = builder.add_skin(Skin {
        joints: vec![bone],
        inverse_bind: vec![Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0))],
        skeleton: Some(bone),
        ..Skin::new("bind")
    });
    builder.model_mut().nodes[root.index()].skin = Some(skin);

    builder.add_animation(Animation {
        name: "wave".into(),
        channels: vec![
            Channel {
                target: bone,
                track: Track::Translation(Curve::linear(vec![0.0, 1.0], vec![Vec3::ZERO, Vec3::X])),
            },
            Channel {
                target: bone,
                track: Track::Rotation(Curve {
                    times: vec![0.0, 2.0],
                    values: vec![Quat::IDENTITY, Quat::from_rotation_y(0.5)],
                    interpolation: Interpolation::Step,
                }),
            },
            Channel {
                target: bone,
                track: Track::Scale(Curve::linear(
                    vec![0.0, 1.0],
                    vec![Vec3::ONE, Vec3::splat(2.0)],
                )),
            },
            Channel {
                target: root,
                track: Track::Weights(Weights {
                    times: vec![0.0, 1.0],
                    values: vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    count: 1,
                    interpolation: Interpolation::CubicSpline,
                }),
            },
        ],
    });

    builder.finish().expect("a model of everything")
}

fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(b"not a real image, only a name for one");
    bytes
}

#[test]
fn third_party_document_reads_correctly() {
    let mut buffer: Vec<u8> = Vec::new();
    for position in [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)] {
        buffer.extend(
            position
                .to_array()
                .iter()
                .flat_map(|value| value.to_le_bytes()),
        );
    }
    for uv in [[0u16, 0], [65535, 0], [0, 65535], [65535, 65535]] {
        buffer.extend(uv.iter().flat_map(|value| value.to_le_bytes()));
    }

    let document = r#"{
      "asset": { "version": "2.0", "generator": "another tool" },
      "buffers": [{ "uri": "shape.bin", "byteLength": 64 }],
      "bufferViews": [
        { "buffer": 0, "byteOffset": 0, "byteLength": 48 },
        { "buffer": 0, "byteOffset": 48, "byteLength": 16 }
      ],
      "accessors": [
        { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
          "min": [0, 0, 0], "max": [1, 1, 0] },
        { "bufferView": 1, "componentType": 5123, "normalized": true, "count": 4, "type": "VEC2" }
      ],
      "images": [{ "uri": "skin%20map.png" }],
      "samplers": [{ "magFilter": 9728, "wrapS": 33071, "wrapT": 33071 }],
      "textures": [{ "source": 0, "sampler": 0 }],
      "materials": [
        { "name": "paint", "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }
      ],
      "meshes": [
        { "name": "strip", "primitives": [
          { "attributes": { "POSITION": 0, "TEXCOORD_0": 1 }, "mode": 5, "material": 0 }
        ]}
      ],
      "nodes": [{ "name": "strip", "mesh": 0 }],
      "scenes": [{ "nodes": [0] }],
      "scene": 0
    }"#;

    let resolver = rikka_model_core::MemoryResolver::new()
        .with_file("shape.bin", buffer)
        .with_file("skin map.png", png());
    let model = Import::named_bytes("shape.gltf", document.as_bytes().to_vec(), registry())
        .resolver(Arc::new(resolver))
        .run()
        .expect("a document another tool wrote");

    assert!(model.warnings.is_empty(), "{:?}", model.warnings);
    assert_eq!(model.metadata.generator.as_deref(), Some("another tool"));

    let mesh = &model.meshes[0];
    assert_eq!(mesh.triangle_count(), 2);
    assert_eq!(mesh.vertices.len(), 6);
    assert_eq!(mesh.vertices.normals.len(), 6);
    assert!(
        mesh.vertices
            .normals
            .iter()
            .all(|normal| *normal == Vec3::Z)
    );
    assert_eq!(mesh.vertices.uvs[1], Vec2::new(1.0, 0.0));

    assert_eq!(model.images[0].name, "skin map.png");
    assert_eq!(model.images[0].bytes.as_ref(), png().as_slice());
    assert_eq!(
        model.textures[0].sampler,
        Sampler::new(
            rikka_model_core::Filter::Nearest,
            rikka_model_core::Wrap::Clamp
        )
    );
    assert_eq!(model.materials[0].name, "paint");
    assert_eq!(
        model.materials[0].textures.base_color.map(|id| id.index()),
        Some(0)
    );
}

#[test]
fn khr_material_extensions_read_into_layers() {
    let document = r#"{
      "asset": { "version": "2.0" },
      "extensionsUsed": ["KHR_materials_clearcoat", "KHR_materials_sheen",
        "KHR_materials_transmission", "KHR_materials_volume", "KHR_materials_ior",
        "KHR_materials_specular", "KHR_materials_iridescence", "KHR_materials_anisotropy",
        "KHR_materials_emissive_strength", "KHR_materials_dispersion"],
      "materials": [
        { "name": "layered", "emissiveFactor": [1, 0.5, 0],
          "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 0.4 },
          "extensions": {
            "KHR_materials_clearcoat": { "clearcoatFactor": 0.8, "clearcoatRoughnessFactor": 0.1 },
            "KHR_materials_sheen": { "sheenColorFactor": [0.9, 0.2, 0.3], "sheenRoughnessFactor": 0.6 },
            "KHR_materials_transmission": { "transmissionFactor": 0.7 },
            "KHR_materials_volume": { "thicknessFactor": 0.3, "attenuationDistance": 0.5,
              "attenuationColor": [0.9, 0.8, 0.7] },
            "KHR_materials_ior": { "ior": 1.7 },
            "KHR_materials_specular": { "specularFactor": 0.6, "specularColorFactor": [1, 0.9, 0.8] },
            "KHR_materials_iridescence": { "iridescenceFactor": 1.0, "iridescenceIor": 1.4,
              "iridescenceThicknessMinimum": 150, "iridescenceThicknessMaximum": 450 },
            "KHR_materials_anisotropy": { "anisotropyStrength": 0.6, "anisotropyRotation": 1.2 },
            "KHR_materials_emissive_strength": { "emissiveStrength": 4.0 },
            "KHR_materials_dispersion": { "dispersion": 0.2 }
          } },
        { "name": "window", "extensions": { "KHR_materials_transmission": { "transmissionFactor": 1.0 } } }
      ]
    }"#;
    let model = Import::named_bytes("layered.gltf", document.as_bytes().to_vec(), registry())
        .run()
        .expect("a document with layered materials");
    assert!(model.warnings.is_empty(), "{:?}", model.warnings);

    let layered = &model.materials[0];
    let Shading::Pbr(pbr) = layered.shading else {
        panic!("glTF materials read as metallic-roughness");
    };
    assert_eq!(layered.emissive, Vec3::new(4.0, 2.0, 0.0));
    assert_eq!(pbr.roughness, 0.4);
    assert_eq!(
        (
            pbr.coat.weight,
            pbr.coat.roughness,
            pbr.coat.ior,
            pbr.coat.darkening
        ),
        (0.8, 0.1, 1.5, 0.0)
    );
    assert_eq!(
        (pbr.sheen.weight, pbr.sheen.color, pbr.sheen.roughness),
        (1.0, Vec3::new(0.9, 0.2, 0.3), 0.6)
    );
    assert_eq!(pbr.transmission.weight, 0.7);
    assert_eq!(pbr.transmission.thickness, 0.3);
    assert_eq!(pbr.transmission.attenuation_distance, 0.5);
    assert_eq!(pbr.transmission.attenuation_color, Vec3::new(0.9, 0.8, 0.7));
    assert_eq!(pbr.transmission.dispersion, 0.2);
    assert!(!pbr.thin_walled, "a volume gives the glass a thickness");
    assert_eq!(
        (pbr.specular.ior, pbr.specular.weight, pbr.specular.color),
        (1.7, 0.6, Vec3::new(1.0, 0.9, 0.8))
    );
    assert_eq!(
        (
            pbr.iridescence.weight,
            pbr.iridescence.ior,
            pbr.iridescence.thickness_min,
            pbr.iridescence.thickness_max
        ),
        (1.0, 1.4, 150.0, 450.0)
    );
    assert_eq!(
        (pbr.anisotropy.strength, pbr.anisotropy.rotation),
        (0.6, 1.2)
    );

    let Shading::Pbr(window) = model.materials[1].shading else {
        panic!("glTF materials read as metallic-roughness");
    };
    assert!(
        window.thin_walled,
        "transmission without a volume is a thin sheet"
    );
}

#[test]
fn unused_skin_is_not_written() {
    let mut builder = ModelBuilder::new().importer("test");
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2],
        ..Mesh::new("triangle")
    });
    let skin = builder.add_skin(Skin::new("empty"));
    let node = builder.add_node(Node::new("body").mesh(mesh));
    builder.model_mut().nodes[node.index()].skin = Some(skin);
    let model = builder.finish().expect("a model with an empty skin");

    let again = written(&model, "empty.glb", ExportOptions::new());
    assert!(again.skins.is_empty());
    assert!(again.node(again.roots[0]).skin.is_none());
    assert_eq!(again.meshes.len(), 1);
    assert_eq!(again.meshes[0].triangle_count(), 1);
}
