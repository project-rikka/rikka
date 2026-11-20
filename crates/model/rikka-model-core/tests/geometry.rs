use rikka_model_core::glam::{Mat4, Quat, Vec3, Vec4};
use rikka_model_core::{
    Animation, Camera, Channel, Conversion, CoordinateSystem, Corner, Curve, Light, LightKind,
    MaterialId, Mesh, MeshAssembler, MeshId, Model, ModelBuilder, MorphTarget, Node, Projection,
    Repeats, Skin, Track, Transform, Vertices, convert_coordinates,
};

use crate::support::triangle;

fn left_handed_model() -> Model {
    let mut builder = ModelBuilder::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(0.1);
    let mut mesh = triangle();
    mesh.vertices.positions[2] = Vec3::Z;
    mesh.vertices.normals = vec![Vec3::Z; 3];
    mesh.vertices.tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 3];
    let mesh = builder.add_mesh(mesh);
    let node = builder.add_node(
        Node::new("floor")
            .transform(Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)))
            .mesh(mesh),
    );
    let skin = builder.add_skin(Skin {
        joints: vec![node],
        inverse_bind: vec![Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0))],
        ..Skin::new("skin")
    });
    builder.model_mut().nodes[node.index()].skin = Some(skin);
    builder.add_animation(Animation {
        name: "wave".into(),
        channels: vec![
            Channel {
                target: node,
                track: Track::Translation(Curve::linear(vec![0.0], vec![Vec3::Z])),
            },
            Channel {
                target: node,
                track: Track::Rotation(Curve::linear(vec![0.0], vec![Quat::from_rotation_y(0.5)])),
            },
        ],
    });
    builder.finish().expect("a left handed model")
}

#[test]
fn left_handed_conversion_mirrors_once() {
    let mut model = left_handed_model();

    convert_coordinates(&mut model, CoordinateSystem::CANONICAL, 0.1);

    assert_eq!(model.metadata.coordinates, CoordinateSystem::CANONICAL);
    let vertices = &model.meshes[0].vertices;
    assert_eq!(vertices.positions[2], Vec3::new(0.0, 0.0, -0.1));
    assert!(vertices.normals.iter().all(|normal| *normal == Vec3::NEG_Z));
    assert_eq!(vertices.tangents[0], Vec4::new(1.0, 0.0, 0.0, -1.0));
    assert_eq!(model.meshes[0].indices, vec![0, 2, 1]);
    assert_eq!(
        model.nodes[0].transform.translation,
        Vec3::new(0.0, 0.0, -0.2)
    );
    assert_eq!(
        model.skins[0].inverse_bind[0].w_axis.truncate(),
        Vec3::new(0.0, 0.0, -0.3)
    );
    let Track::Translation(moved) = &model.animations[0].channels[0].track else {
        panic!("the first channel moves the node");
    };
    assert_eq!(moved.values[0], Vec3::new(0.0, 0.0, -0.1));
    let Track::Rotation(turned) = &model.animations[0].channels[1].track else {
        panic!("the second channel turns the node");
    };
    assert!(turned.values[0].abs_diff_eq(Quat::from_rotation_y(-0.5), 1e-5));

    let before = model.meshes[0].vertices.positions.clone();
    convert_coordinates(&mut model, CoordinateSystem::CANONICAL, 1.0);
    assert_eq!(model.meshes[0].vertices.positions, before);
}

#[test]
fn z_up_conversion_keeps_scale() {
    let upright = Conversion::new(
        CoordinateSystem::RIGHT_HANDED_Z_UP,
        CoordinateSystem::CANONICAL,
        1.0,
    );
    assert!(!upright.flips_winding());
    assert_eq!(upright.point(Vec3::Z), Vec3::Y);

    let mirror = Conversion::new(
        CoordinateSystem::LEFT_HANDED_Y_UP,
        CoordinateSystem::CANONICAL,
        1.0,
    );
    assert!(mirror.flips_winding());
    assert_eq!(
        mirror.scaling(Vec3::new(1.0, 2.0, 3.0)),
        Vec3::new(1.0, 2.0, 3.0)
    );

    let mut target = MorphTarget {
        name: "smile".into(),
        vertices: vec![0],
        positions: vec![Vec3::Z],
        normals: vec![Vec3::Z],
    };
    target.convert(&Conversion::new(
        CoordinateSystem::CANONICAL,
        CoordinateSystem::CANONICAL,
        0.1,
    ));
    assert_eq!(target.positions[0], Vec3::new(0.0, 0.0, 0.1));
    assert_eq!(target.normals[0], Vec3::Z);
}

#[test]
fn conversion_scales_lenses_and_keeps_flattened_nodes_finite() {
    let mut builder = ModelBuilder::new().coordinates(CoordinateSystem::RIGHT_HANDED_Z_UP);
    builder.add_node(Node::new("flat").transform(Transform {
        translation: Vec3::new(1.0, 2.0, 3.0),
        rotation: Quat::from_rotation_x(0.3),
        scale: Vec3::new(1.0, 0.0, 2.0),
    }));
    builder.add_camera(Camera::new("lens").projection(Projection::Orthographic {
        width: 2.0,
        height: 4.0,
        near: 1.0,
        far: 10.0,
    }));
    builder.add_light(Light::new("lamp", LightKind::Point { range: 8.0 }));
    let mut model = builder.finish().expect("a valid model");
    convert_coordinates(&mut model, CoordinateSystem::CANONICAL, 0.5);

    let flat = model.nodes[0].transform;
    assert!(flat.rotation.is_normalized(), "{:?}", flat.rotation);
    assert_eq!(flat.scale, Vec3::new(1.0, 2.0, 0.0));
    assert_eq!(flat.translation, Vec3::new(0.5, 1.5, -1.0));
    assert_eq!(
        model.cameras[0].projection,
        Projection::Orthographic {
            width: 1.0,
            height: 2.0,
            near: 0.5,
            far: 5.0,
        }
    );
    assert_eq!(model.lights[0].kind, LightKind::Point { range: 4.0 });

    let decomposed = Transform::from_matrix(Mat4::from_scale(Vec3::new(1.0, 0.0, 2.0)));
    assert!(decomposed.rotation.is_normalized());
    assert_eq!(decomposed.scale, Vec3::new(1.0, 0.0, 2.0));
}

#[test]
fn skin_weights_keep_four_strongest() {
    let mut vertices = Vertices {
        positions: vec![Vec3::ZERO; 4],
        joints: vec![[1, 2, 0, 0], [3, 4, 5, 6], [0; 4], [0; 4]],
        weights: vec![Vec4::new(0.5, 0.25, 0.0, 0.0), Vec4::ZERO, Vec4::X, Vec4::X],
        ..Vertices::default()
    };
    vertices.normalize_weights();
    assert!((vertices.weights[0].element_sum() - 1.0).abs() < 1e-6);
    assert_eq!(vertices.joints[0], [1, 2, 0, 0]);
    assert_eq!(vertices.weights[1], Vec4::X, "bound to nothing falls back");
    assert_eq!(vertices.joints[1], [3, 0, 0, 0]);

    vertices.set_influences(2, &[(7, 0.25), (3, 0.75)]);
    vertices.set_influences(
        3,
        &[(1, 0.1), (2, 0.3), (3, 0.2), (4, 0.3), (5, 0.05), (6, 0.05)],
    );
    assert_eq!(
        vertices.influences(2),
        Some([(7, 0.25), (3, 0.75), (0, 0.0), (0, 0.0)])
    );
    assert_eq!(
        vertices.influences(3),
        Some([(2, 0.3), (4, 0.3), (3, 0.2), (1, 0.1)])
    );
    assert_eq!(vertices.influences(4), None);
}

#[test]
fn morph_target_merge_sorts_and_resolves_repeats() {
    let mut first = MorphTarget::new("smile");
    let mut summed = MorphTarget::new("smile");
    for (vertex, offset) in [(4, Vec3::X), (1, Vec3::Y), (4, Vec3::Z), (0, Vec3::ZERO)] {
        first.push(vertex, offset, Some(offset));
        summed.push(vertex, offset, None);
    }

    first.merge(Repeats::KeepFirst);
    summed.merge(Repeats::Sum);

    assert_eq!(first.vertices, vec![0, 1, 4]);
    assert_eq!(first.positions, vec![Vec3::ZERO, Vec3::Y, Vec3::X]);
    assert_eq!(first.normals, vec![Vec3::ZERO, Vec3::Y, Vec3::X]);
    assert_eq!(summed.vertices, vec![0, 1, 4]);
    assert_eq!(
        summed.positions,
        vec![Vec3::ZERO, Vec3::Y, Vec3::X + Vec3::Z]
    );
    assert!(summed.normals.is_empty());
}

#[test]
fn child_inherits_parent_transform() {
    let mut builder = ModelBuilder::new();
    let root = builder.add_node(Node::new("root").transform(Transform::from_translation(Vec3::X)));
    let child = builder.add_node(
        Node::new("child").transform(Transform::from_translation(Vec3::new(0.0, 2.0, 0.0))),
    );
    builder.attach(root, child);
    let model = builder.finish().expect("a chain of two nodes");

    let world = model.world_transforms();

    assert_eq!(
        world[child.index()].w_axis.truncate(),
        Vec3::new(1.0, 2.0, 0.0)
    );
}

type Breakage = (&'static str, fn(&mut Model));
#[test]
fn validate_rejects_dangling_ids_and_cycles() {
    let valid = || {
        let mut builder = ModelBuilder::new();
        let mesh = builder.add_mesh(triangle());
        let root = builder.add_node(Node::new("root").mesh(mesh));
        let child = builder.add_node(Node::new("child"));
        builder.attach(root, child);
        builder.finish().expect("a valid model")
    };
    let broken: [Breakage; 11] = [
        ("an index past the vertices", |model| {
            model.meshes[0].indices = vec![0, 1, 7];
        }),
        ("a submesh past the indices", |model| {
            model.meshes[0].submeshes[0].index_count = 6;
        }),
        ("a submesh naming a missing material", |model| {
            model.meshes[0].submeshes[0].material = Some(MaterialId(2));
        }),
        ("a node naming a missing mesh", |model| {
            model.nodes[1].mesh = Some(MeshId(3));
        }),
        ("a looped hierarchy", |model| {
            model.nodes[0].parent = Some(rikka_model_core::NodeId(1));
            model.nodes[1].children.push(rikka_model_core::NodeId(0));
            model.roots.clear();
        }),
        ("a morph target naming a vertex twice", |model| {
            let mut target = MorphTarget::new("smile");
            target.push(1, Vec3::X, None);
            target.push(1, Vec3::Y, None);
            model.meshes[0].morph_targets.push(target);
        }),
        ("a curve with fewer values than keyframes", |model| {
            model.animations.push(Animation {
                name: "short".into(),
                channels: vec![Channel {
                    target: rikka_model_core::NodeId(0),
                    track: Track::Translation(Curve::linear(vec![0.0, 1.0], vec![Vec3::ZERO])),
                }],
            });
        }),
        ("keyframes going backwards in time", |model| {
            model.animations.push(Animation {
                name: "backwards".into(),
                channels: vec![Channel {
                    target: rikka_model_core::NodeId(0),
                    track: Track::Translation(Curve::linear(
                        vec![1.0, 0.0],
                        vec![Vec3::ZERO, Vec3::X],
                    )),
                }],
            });
        }),
        ("a keyframe at no time", |model| {
            model.animations.push(Animation {
                name: "timeless".into(),
                channels: vec![Channel {
                    target: rikka_model_core::NodeId(0),
                    track: Track::Translation(Curve::linear(vec![f32::NAN], vec![Vec3::ZERO])),
                }],
            });
        }),
        ("a root naming a parent", |model| {
            model.nodes[0].parent = Some(rikka_model_core::NodeId(9));
        }),
        ("a node its parent does not list", |model| {
            model.nodes[0].children.clear();
            model.roots.push(rikka_model_core::NodeId(1));
        }),
    ];

    assert!(valid().validate().is_ok());
    for (what, break_it) in broken {
        let mut model = valid();
        break_it(&mut model);
        assert!(model.validate().is_err(), "{what} was accepted");
    }
}

#[test]
fn finishing_reports_bad_indices() {
    let mut mesh = triangle();
    mesh.indices = vec![0, 1, 7];
    let mut builder = ModelBuilder::new();
    builder.add_mesh(mesh);
    assert!(builder.finish().is_err());
}

fn split_cube() -> Mesh {
    let mut mesh = Mesh::new("cube");
    for axis in 0..3 {
        for side in [-1.0f32, 1.0] {
            let mut normal = Vec3::ZERO;
            normal[axis] = side;
            let u = Vec3::from_array([0, 1, 2].map(|i| f32::from(u8::from(i == (axis + 1) % 3))));
            let v = normal.cross(u);
            let first = mesh.vertices.positions.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                mesh.vertices.positions.push(normal + u * a + v * b);
                mesh.vertices.normals.push(normal);
            }
            mesh.indices
                .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }
    mesh
}

#[test]
fn smoothed_normals_weld_split_corners() {
    let cube = split_cube();
    let smoothed = cube.smoothed_normals();
    assert_eq!(smoothed.len(), cube.vertices.positions.len());
    for (position, normal) in cube.vertices.positions.iter().zip(&smoothed) {
        assert!(
            normal.abs_diff_eq(position.normalize(), 1e-5),
            "corner {position} should point along its diagonal, got {normal}"
        );
    }
}

#[test]
fn assembler_records_polygons_and_conversion_flips_them() {
    let mut assembler = MeshAssembler::new();
    let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let vertices: Vec<u32> = square
        .iter()
        .enumerate()
        .map(|(point, [x, y])| {
            assembler.corner(point as u32, Corner::new(Vec3::new(*x, *y, 0.0), Vec3::Z))
        })
        .collect();
    assembler.polygon(&vertices, 3);
    let mut mesh = assembler.finish("square").mesh;
    let polygons = mesh.polygons.clone().expect("polygons are recorded");
    assert_eq!(polygons.offsets, vec![0, 4]);
    assert_eq!(polygons.corner_vertices, vec![0, 1, 2, 3]);
    assert_eq!(polygons.vertex_points, vec![0, 1, 2, 3]);
    assert_eq!(polygons.submeshes, vec![0]);
    mesh.flip_winding();
    assert_eq!(
        mesh.polygons
            .as_ref()
            .map(|polygons| polygons.polygon(0).to_vec()),
        Some(vec![0, 3, 2, 1])
    );

    let mut builder = ModelBuilder::new();
    let mut broken = mesh.clone();
    if let Some(polygons) = &mut broken.polygons {
        polygons.corner_vertices[1] = 9;
    }
    builder.add_mesh(broken);
    assert!(builder.finish().is_err());

    let mut triangles = MeshAssembler::new();
    let corners = [Vec3::ZERO, Vec3::X, Vec3::Y]
        .map(|position| triangles.corner(0, Corner::new(position, Vec3::Z)));
    triangles.triangle(corners, 0);
    assert!(triangles.finish("triangle").mesh.polygons.is_none());
}
