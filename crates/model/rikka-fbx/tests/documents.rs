use std::sync::Arc;

use rikka_fbx::FbxExporter;
use rikka_model_core::glam::{Vec3, Vec4};
use rikka_model_core::{
    Alpha, Export, ExportOptions, Import, ImportOptions, MemoryResolver, MemorySink, Model,
    Shading, Sink, TextureChannel, Track,
};

use crate::support::{KNIGHT, registry};

const TEXTURE: &[u8] = include_bytes!("../assets/knight_texture.png");

fn knight() -> Model {
    Import::named_bytes("knight.fbx", KNIGHT.to_vec(), registry())
        .resolver(Arc::new(
            MemoryResolver::new().with_file("knight_texture.png", TEXTURE.to_vec()),
        ))
        .run()
        .expect("the sample knight is read")
}

fn written(model: &Model, name: &str) -> Model {
    let sink = Arc::new(MemorySink::new());
    let exported = Export::new(model, registry())
        .options(ExportOptions::new().embed_resources(true))
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes(name)
        .expect("the model is written");
    assert!(exported.warnings.is_empty(), "{:?}", exported.warnings);
    Import::named_bytes(name, exported.bytes, registry())
        .run()
        .expect("the written model is read again")
}

#[test]
fn sample_document_reads() {
    let model = knight();
    assert_eq!(model.metadata.importer, "FBX");
    assert!(!model.meshes.is_empty(), "the knight has meshes");
    assert!(!model.materials.is_empty(), "the knight has materials");
    assert!(model.triangle_count() > 1000, "{}", model.triangle_count());
    model.validate().expect("the model holds together");
}

#[test]
fn document_converts_to_canonical_metres() {
    let model = knight();
    assert_eq!(
        model.metadata.coordinates,
        rikka_model_core::CoordinateSystem::CANONICAL
    );
    assert_eq!(model.metadata.unit_scale, 1.0);

    let bounds = model.bounds();
    let height = bounds.max.y - bounds.min.y;
    assert!((0.5..4.0).contains(&height), "{height} metres tall");
}

#[test]
fn reads_texture_through_windows_path() {
    let model = knight();
    assert!(
        model.warnings.is_empty(),
        "nothing is missing: {:?}",
        model.warnings
    );
    assert_eq!(model.images.len(), 1);
    assert_eq!(model.images[0].bytes.as_ref(), TEXTURE);
    assert!(
        model
            .materials
            .iter()
            .any(|material| material.textures.base_color.is_some())
    );
}

#[test]
fn skinned_document_binds_mesh() {
    let model = knight();
    assert!(!model.skins.is_empty(), "the knight is skinned");
    let skin = &model.skins[0];
    assert!(skin.joint_count() > 10, "{}", skin.joint_count());
    assert!(
        model
            .nodes
            .iter()
            .any(|node| node.skin.is_some() && node.mesh.is_some())
    );
    let mesh = model
        .meshes
        .iter()
        .find(|mesh| mesh.vertices.is_skinned())
        .expect("a skinned mesh");
    assert!(
        mesh.vertices
            .weights
            .iter()
            .all(|weight| (weight.element_sum() - 1.0).abs() < 1e-3)
    );
}

#[test]
fn takes_become_zero_based_animations() {
    let model = knight();
    assert!(model.animations.len() > 10, "{}", model.animations.len());
    for animation in &model.animations {
        assert!(!animation.channels.is_empty(), "{}", animation.name);
        assert!(animation.duration() > 0.0, "{}", animation.name);
        for channel in &animation.channels {
            let first = match &channel.track {
                Track::Translation(curve) | Track::Scale(curve) => curve.times.first().copied(),
                Track::Rotation(curve) => curve.times.first().copied(),
                Track::Weights(weights) => weights.times.first().copied(),
            };
            assert_eq!(first, Some(0.0), "{}", animation.name);
        }
    }
}

#[test]
fn animation_moves_driven_bones() {
    let model = knight();
    let animation = &model.animations[0];
    let Track::Rotation(curve) = &animation
        .channels
        .iter()
        .find_map(|channel| match &channel.track {
            Track::Rotation(_) => Some(&channel.track),
            _ => None,
        })
        .expect("a bone that turns")
    else {
        unreachable!()
    };
    let start = curve.sample(0.0).expect("the first pose");
    let middle = curve.sample(animation.duration() * 0.5).expect("a pose");
    assert!(start.is_normalized() && middle.is_normalized());
}

#[test]
fn model_round_trips() {
    let model = knight();
    let again = written(&model, "knight.fbx");

    assert_eq!(again.nodes.len(), model.nodes.len());
    assert_eq!(again.meshes.len(), model.meshes.len());
    assert_eq!(again.materials.len(), model.materials.len());
    assert_eq!(again.skins.len(), model.skins.len());
    assert_eq!(again.animations.len(), model.animations.len());
    assert_eq!(again.triangle_count(), model.triangle_count());
    assert_eq!(again.images.len(), model.images.len());

    let bounds = model.bounds();
    let reread_bounds = again.bounds();
    assert!(
        (bounds.min - reread_bounds.min).length() < 1e-3,
        "{bounds:?} {reread_bounds:?}"
    );
    assert!(
        (bounds.max - reread_bounds.max).length() < 1e-3,
        "{bounds:?} {reread_bounds:?}"
    );
}

#[test]
fn text_matches_binary() {
    let model = knight();
    let text = registry()
        .exporter_named("FBX Text")
        .expect("the text exporter")
        .info()
        .name;
    let sink = Arc::new(MemorySink::new());
    let exported = Export::with_exporter(&model, Arc::new(FbxExporter::text()))
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .to_bytes("knight.fbx")
        .expect("the model is written");
    assert_eq!(text, "FBX Text");
    assert!(exported.bytes.starts_with(b"; FBX 7.4.0 project file"));

    let again = Import::named_bytes("knight.fbx", exported.bytes, registry())
        .run()
        .expect("the written model is read again");
    assert_eq!(again.nodes.len(), model.nodes.len());
    assert_eq!(again.triangle_count(), model.triangle_count());
    assert_eq!(again.animations.len(), model.animations.len());
}

#[test]
fn material_colour_and_roughness_round_trip() {
    let model = knight();
    let again = written(&model, "knight.fbx");
    for (before, after) in model.materials.iter().zip(&again.materials) {
        assert!(
            (before.base_color - after.base_color).length() < 1e-3,
            "{} against {}",
            before.base_color,
            after.base_color
        );
        let (Shading::Pbr(one), Shading::Pbr(two)) = (before.shading, after.shading) else {
            panic!("physical surfaces");
        };
        assert!((one.roughness - two.roughness).abs() < 1e-2);
        assert!((one.metallic - two.metallic).abs() < 1e-2);
    }
}

#[test]
fn import_honours_target_coordinates_and_scale() {
    let plain = knight();
    let turned = Import::named_bytes("knight.fbx", KNIGHT.to_vec(), registry())
        .options(
            ImportOptions::new()
                .coordinates(rikka_model_core::CoordinateSystem::RIGHT_HANDED_Z_UP)
                .scale(2.0),
        )
        .run()
        .expect("the knight in another system");

    let plain_bounds = plain.bounds();
    let other = turned.bounds();
    assert!(
        ((other.max.z - other.min.z) - (plain_bounds.max.y - plain_bounds.min.y) * 2.0).abs()
            < 1e-2,
        "{plain_bounds:?} {other:?}"
    );
}

#[test]
fn legacy_document_reads_like_current() {
    let text = "; FBX 6.1.0 project file\n\
        FBXHeaderExtension:  {\n\
        \tFBXHeaderVersion: 1003\n\
        \tFBXVersion: 6100\n\
        }\n\
        GlobalSettings:  {\n\
        \tProperties60:  {\n\
        \t\tProperty: \"UpAxis\", \"int\", \"\",1\n\
        \t\tProperty: \"UpAxisSign\", \"int\", \"\",1\n\
        \t\tProperty: \"FrontAxis\", \"int\", \"\",2\n\
        \t\tProperty: \"FrontAxisSign\", \"int\", \"\",1\n\
        \t\tProperty: \"CoordAxis\", \"int\", \"\",0\n\
        \t\tProperty: \"CoordAxisSign\", \"int\", \"\",1\n\
        \t\tProperty: \"UnitScaleFactor\", \"double\", \"\",100\n\
        \t}\n\
        }\n\
        Objects:  {\n\
        \tModel: \"Model::body\", \"Mesh\" {\n\
        \t\tProperties60:  {\n\
        \t\t\tProperty: \"Lcl Translation\", \"Lcl Translation\", \"A+\",0,1,0\n\
        \t\t}\n\
        \t\tVertices: 0,0,0,1,0,0,0,1,0\n\
        \t\tPolygonVertexIndex: 0,1,-3\n\
        \t\tLayerElementMaterial: 0 {\n\
        \t\t\tMappingInformationType: \"AllSame\"\n\
        \t\t\tReferenceInformationType: \"IndexToDirect\"\n\
        \t\t\tMaterials: 0\n\
        \t\t}\n\
        \t}\n\
        \tMaterial: \"Material::red\", \"\" {\n\
        \t\tProperties60:  {\n\
        \t\t\tProperty: \"DiffuseColor\", \"ColorRGB\", \"A\",1,0,0\n\
        \t\t\tProperty: \"TransparencyFactor\", \"double\", \"A\",0.5\n\
        \t\t}\n\
        \t}\n\
        }\n\
        Connections:  {\n\
        \tConnect: \"OO\", \"Material::red\", \"Model::body\"\n\
        }\n\
        Takes:  {\n\
        \tCurrent: \"walk\"\n\
        \tTake: \"walk\" {\n\
        \t\tLocalTime: 0,46186158000\n\
        \t\tModel: \"Model::body\" {\n\
        \t\t\tChannel: \"Transform\" {\n\
        \t\t\t\tChannel: \"T\" {\n\
        \t\t\t\t\tChannel: \"X\" {\n\
        \t\t\t\t\t\tDefault: 0\n\
        \t\t\t\t\t\tKeyVer: 4005\n\
        \t\t\t\t\t\tKeyCount: 2\n\
        \t\t\t\t\t\tKey: 0,0,L,46186158000,4,L\n\
        \t\t\t\t\t}\n\
        \t\t\t\t}\n\
        \t\t\t}\n\
        \t\t}\n\
        \t}\n\
        }\n";

    let model = Import::named_bytes("body.fbx", text.as_bytes().to_vec(), registry())
        .run()
        .expect("an older document is read");

    assert_eq!(model.meshes.len(), 1);
    assert_eq!(model.meshes[0].triangle_count(), 1);
    assert_eq!(model.materials.len(), 1);
    assert_eq!(model.materials[0].base_color, Vec4::new(1.0, 0.0, 0.0, 0.5));
    assert_eq!(model.materials[0].alpha, Alpha::Blend);
    assert_eq!(
        model.nodes[0].transform.translation,
        Vec3::new(0.0, 1.0, 0.0)
    );

    assert_eq!(model.animations.len(), 1);
    let Track::Translation(curve) = &model.animations[0].channels[0].track else {
        panic!("a movement");
    };
    assert_eq!(curve.sample(1.0), Some(Vec3::new(4.0, 1.0, 0.0)));
}

#[test]
fn standard_surface_layers_and_their_maps_are_read() {
    let text = "; FBX 7.4.0 project file\n\
        FBXHeaderExtension:  {\n\
        \tFBXHeaderVersion: 1003\n\
        \tFBXVersion: 7400\n\
        }\n\
        Objects:  {\n\
        \tGeometry: 10, \"Geometry::body\", \"Mesh\" {\n\
        \t\tVertices: *9 {\n\
        \t\t\ta: 0,0,0,1,0,0,0,1,0\n\
        \t\t}\n\
        \t\tPolygonVertexIndex: *3 {\n\
        \t\t\ta: 0,1,-3\n\
        \t\t}\n\
        \t}\n\
        \tModel: 20, \"Model::body\", \"Mesh\" {\n\
        \t}\n\
        \tMaterial: 30, \"Material::lacquer\", \"\" {\n\
        \t\tProperties70:  {\n\
        \t\t\tP: \"Maya|coat\", \"float\", \"\", \"A\",0.8\n\
        \t\t\tP: \"Maya|coatRoughness\", \"float\", \"\", \"A\",0.2\n\
        \t\t\tP: \"Maya|sheen\", \"float\", \"\", \"A\",1\n\
        \t\t\tP: \"Maya|sheenColor\", \"Color\", \"\", \"A\",0.9,0.2,0.3\n\
        \t\t\tP: \"Maya|transmission\", \"float\", \"\", \"A\",0.5\n\
        \t\t\tP: \"Maya|specularIOR\", \"float\", \"\", \"A\",1.7\n\
        \t\t}\n\
        \t}\n\
        \tTexture: 40, \"Texture::mask\", \"\" {\n\
        \t\tRelativeFilename: \"mask.png\"\n\
        \t}\n\
        \tVideo: 50, \"Video::mask\", \"Clip\" {\n\
        \t\tRelativeFilename: \"mask.png\"\n\
        \t}\n\
        }\n\
        Connections:  {\n\
        \tC: \"OO\",10,20\n\
        \tC: \"OO\",20,0\n\
        \tC: \"OO\",30,20\n\
        \tC: \"OO\",50,40\n\
        \tC: \"OP\",40,30, \"Maya|coat\"\n\
        \tC: \"OP\",40,30, \"Maya|sheenRoughness\"\n\
        }\n";
    let model = Import::named_bytes("lacquer.fbx", text.as_bytes().to_vec(), registry())
        .resolver(Arc::new(
            MemoryResolver::new().with_file("mask.png", TEXTURE.to_vec()),
        ))
        .run()
        .expect("a standard surface document");
    let material = &model.materials[0];
    let Shading::Pbr(pbr) = material.shading else {
        panic!("FBX materials read as metallic-roughness");
    };
    assert_eq!((pbr.coat.weight, pbr.coat.roughness), (0.8, 0.2));
    assert_eq!(
        (pbr.sheen.weight, pbr.sheen.color),
        (1.0, Vec3::new(0.9, 0.2, 0.3))
    );
    assert_eq!(pbr.transmission.weight, 0.5);
    assert_eq!(pbr.specular.ior, 1.7);
    assert_eq!(material.alpha, Alpha::Blend, "transmission blends");
    let textures = &material.textures;
    assert!(textures.coat.is_some() && textures.coat == textures.sheen_roughness);
    assert_eq!(textures.channels.coat, TextureChannel::Red);
    assert_eq!(textures.channels.sheen_roughness, TextureChannel::Red);
}

#[test]
fn skinned_mesh_keeps_bind_pose() {
    let model = knight();
    let world = model.world_transforms();
    let mut checked = 0;

    for (index, node) in model.nodes.iter().enumerate() {
        let (Some(mesh), Some(skin)) = (node.mesh, node.skin) else {
            continue;
        };
        let node_world = world[index];
        let mesh = model.mesh(mesh);
        let skin = model.skin(skin);
        let palette: Vec<rikka_model_core::glam::Mat4> = skin
            .joints
            .iter()
            .zip(&skin.inverse_bind)
            .map(|(joint, bind)| world[joint.index()] * *bind)
            .collect();

        for (vertex, position) in mesh.vertices.positions.iter().enumerate().step_by(37) {
            let joints = mesh.vertices.joints[vertex];
            let weights = mesh.vertices.weights[vertex];
            let mut skinned_position = rikka_model_core::glam::Vec3::ZERO;
            for slot in 0..4 {
                skinned_position += (palette[usize::from(joints[slot])] * position.extend(1.0))
                    .truncate()
                    * weights[slot];
            }
            let rigid_position = node_world.transform_point3(*position);
            assert!(
                (skinned_position - rigid_position).length() < 1e-3,
                "node {index} vertex {vertex}: {skinned_position} against {rigid_position}"
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked} vertices of the knight are bound");
}

#[test]
fn document_exposes_props() {
    let model = knight();
    let named = |name: &str| model.nodes.iter().any(|node| node.name == name);
    assert!(named("Knight_Body") && named("Knight_Head"));
    assert!(named("2H_Sword") && named("Round_Shield"));
    assert!(
        model
            .nodes
            .iter()
            .filter(|node| node.mesh.is_some())
            .count()
            >= 15
    );
}

fn assembled() -> Model {
    use rikka_model_core::glam::{Mat4, Quat, Vec2};
    use rikka_model_core::{
        Animation, Camera, Channel, Curve, Image, Interpolation, Light, LightKind, Material,
        MaterialTextures, Mesh, ModelBuilder, MorphTarget, Node, Projection, Skin, Submesh,
        Texture, Transform, Vertices, Weights,
    };

    let mut builder = ModelBuilder::new();
    let image = builder.add_image(Image::new("skin.png", TEXTURE.to_vec()));
    let texture = builder.add_texture(Texture::new(image).named("skin"));
    let material = builder.add_material(
        Material::new("skin")
            .base_color(Vec4::new(0.8, 0.4, 0.2, 1.0))
            .textures(MaterialTextures {
                base_color: Some(texture),
                ..MaterialTextures::default()
            }),
    );
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
            normals: vec![Vec3::Z; 4],
            uvs: vec![Vec2::ZERO, Vec2::X, Vec2::Y, Vec2::new(1.0, 1.0)],
            colors: vec![[255, 128, 0, 255]; 4],
            joints: vec![[0, 0, 0, 0], [1, 0, 0, 0], [0, 0, 0, 0], [1, 0, 0, 0]],
            weights: vec![Vec4::X; 4],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2, 1, 3, 2],
        submeshes: vec![Submesh {
            index_count: 6,
            material: Some(material),
            ..Submesh::default()
        }],
        morph_targets: vec![MorphTarget {
            name: "smile".into(),
            vertices: vec![1, 3],
            positions: vec![Vec3::Z, Vec3::Z * 2.0],
            normals: Vec::new(),
        }],
        ..Mesh::new("quad")
    });

    let root = builder.add_node(Node::new("root").mesh(mesh).morph_weights([0.25]));
    let bone = builder.add_node(Node {
        parent: Some(root),
        ..Node::new("bone").transform(Transform {
            translation: Vec3::Y,
            rotation: Quat::from_rotation_z(0.4),
            scale: Vec3::splat(1.5),
        })
    });
    let skin = builder.add_skin(Skin {
        joints: vec![root, bone],
        inverse_bind: vec![Mat4::IDENTITY, Mat4::from_translation(Vec3::NEG_Y)],
        ..Skin::new("skin")
    });
    builder.model_mut().nodes[root.index()].skin = Some(skin);

    let camera = builder.add_camera(Camera::new("eye").projection(Projection::Perspective {
        y_fov: 0.8,
        aspect: Some(1.5),
        near: 0.1,
        far: Some(50.0),
    }));
    builder.add_node(Node::new("eye").camera(camera).transform(Transform {
        translation: Vec3::new(0.0, 1.0, 4.0),
        ..Transform::IDENTITY
    }));
    let light = builder.add_light(
        Light::new(
            "lamp",
            LightKind::Spot {
                range: 12.0,
                inner_angle: 0.2,
                outer_angle: 0.6,
            },
        )
        .color(Vec3::new(1.0, 0.9, 0.7))
        .intensity(2.0),
    );
    builder.add_node(Node::new("lamp").light(light));

    builder.add_animation(Animation {
        name: "wave".into(),
        channels: vec![
            Channel {
                target: bone,
                track: Track::Rotation(Curve::linear(
                    vec![0.0, 1.0],
                    vec![Quat::IDENTITY, Quat::from_rotation_x(0.9)],
                )),
            },
            Channel {
                target: root,
                track: Track::Weights(Weights {
                    times: vec![0.0, 1.0],
                    values: vec![0.0, 1.0],
                    count: 1,
                    interpolation: Interpolation::Linear,
                }),
            },
        ],
    });
    builder.finish().expect("a model of every part")
}

#[test]
fn model_round_trips_losslessly() {
    let model = assembled();
    let again = written(&model, "assembled.fbx");

    assert_eq!(again.meshes.len(), 1);
    assert_eq!(again.meshes[0].morph_targets.len(), 1);
    assert_eq!(again.meshes[0].morph_targets[0].name, "smile");
    assert_eq!(again.skins.len(), 1);
    assert_eq!(again.skins[0].joint_count(), 2);
    assert_eq!(again.cameras.len(), 1);
    assert_eq!(again.lights.len(), 1);
    assert_eq!(again.images.len(), 1);
    assert_eq!(again.animations.len(), 1);
    assert_eq!(again.triangle_count(), 2);

    let textured_mesh = again
        .meshes
        .iter()
        .find(|mesh| !mesh.vertices.uvs.is_empty())
        .expect("the coordinates come back");
    assert_eq!(
        textured_mesh.vertices.uvs.len(),
        textured_mesh.vertices.positions.len()
    );
    assert!(
        !textured_mesh.vertices.colors.is_empty(),
        "the colours come back"
    );
}

#[test]
fn camera_and_light_orientation_round_trip() {
    let model = assembled();
    let again = written(&model, "assembled.fbx");

    let facing = |model: &Model, is_target: fn(&rikka_model_core::Node) -> bool| {
        let world = model.world_transforms();
        model
            .nodes
            .iter()
            .enumerate()
            .find(|(_, node)| is_target(node))
            .map(|(index, _)| world[index].transform_vector3(Vec3::NEG_Z).normalize())
            .expect("a node that faces somewhere")
    };
    let camera = |node: &rikka_model_core::Node| node.camera.is_some();
    let light = |node: &rikka_model_core::Node| node.light.is_some();
    assert!((facing(&model, camera) - facing(&again, camera)).length() < 1e-4);
    assert!((facing(&model, light) - facing(&again, light)).length() < 1e-4);

    let rikka_model_core::LightKind::Spot { outer_angle, .. } = again.lights[0].kind else {
        panic!("a spot light");
    };
    assert!((outer_angle - 0.6).abs() < 1e-3, "{outer_angle}");
    assert!((again.lights[0].intensity - 2.0).abs() < 1e-3);
}

#[test]
fn bone_rotation_round_trips() {
    let model = assembled();
    let again = written(&model, "assembled.fbx");
    let rotation_span = |model: &Model| {
        let animation = &model.animations[0];
        let channel = animation
            .channels
            .iter()
            .find(|channel| matches!(channel.track, Track::Rotation(_)))
            .expect("a bone that turns");
        let Track::Rotation(curve) = &channel.track else {
            unreachable!()
        };
        (
            curve.sample(0.0).expect("the first pose"),
            curve.sample(1.0).expect("the last pose"),
        )
    };
    let (before, after) = rotation_span(&model);
    let (one, two) = rotation_span(&again);
    assert!(one.abs_diff_eq(before, 1e-3), "{one} against {before}");
    assert!(two.abs_diff_eq(after, 1e-3), "{two} against {after}");
}

#[test]
fn blend_shapes_round_trip_with_weights() {
    let model = assembled();
    let again = written(&model, "assembled.fbx");
    let node = again
        .nodes
        .iter()
        .find(|node| node.mesh.is_some())
        .expect("the node that draws");
    assert_eq!(node.morph_weights.len(), 1);
    assert!((node.morph_weights[0] - 0.25).abs() < 1e-3);

    let weights = again.animations[0]
        .channels
        .iter()
        .find_map(|channel| match &channel.track {
            Track::Weights(weights) => Some(weights),
            _ => None,
        })
        .expect("a shape that is driven");
    let mut out = [0.0];
    assert!(weights.sample_into(1.0, &mut out));
    assert!((out[0] - 1.0).abs() < 1e-3, "{out:?}");
}
