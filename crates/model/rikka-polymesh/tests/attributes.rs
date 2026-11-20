use glam::{Vec2, Vec3, Vec4};
use rikka_polymesh::{
    Attribute, Attributes, DataType, Domain, DomainSizes, Influence, Polymesh, ShapeKey, Values,
    uv_pin_name,
};

use crate::support::{cube, vec2};

#[test]
fn interpolation_follows_blender_rules_per_type() {
    let cases = [
        (
            Values::Bool(vec![true, false]),
            vec![(0, 0.0), (1, 1.0)],
            Values::Bool(vec![true, false, false]),
        ),
        (
            Values::Bool(vec![true, false]),
            vec![(0, 0.1), (1, 0.9)],
            Values::Bool(vec![true, false, true]),
        ),
        (
            Values::Int(vec![1, 2]),
            vec![(0, 0.5), (1, 0.5)],
            Values::Int(vec![1, 2, 2]),
        ),
        (
            Values::Float(vec![1.0, 3.0]),
            vec![(0, 0.25), (1, 0.75)],
            Values::Float(vec![1.0, 3.0, 2.5]),
        ),
        (
            Values::Color(vec![Vec4::ZERO, Vec4::ONE]),
            vec![(0, 0.5), (1, 0.5)],
            Values::Color(vec![Vec4::ZERO, Vec4::ONE, Vec4::splat(0.5)]),
        ),
    ];
    for (mut values, sources, expected) in cases {
        values.push_interpolated(&sources);
        assert_eq!(values, expected);
    }
}

#[test]
fn resizing_fills_type_defaults() {
    let mut colors = Values::new(DataType::Color, 1);
    colors.resize(2);
    assert_eq!(colors, Values::Color(vec![Vec4::ONE, Vec4::ONE]));
    let mut flags = Values::Bool(vec![true]);
    flags.resize(3);
    assert_eq!(flags, Values::Bool(vec![true, false, false]));
}

#[test]
fn conversion_follows_blender_implicit_rules() {
    assert_eq!(
        Values::Vec3(vec![Vec3::new(1.0, 2.0, 6.0)]).convert(DataType::Float),
        Values::Float(vec![3.0])
    );
    assert_eq!(
        Values::Float(vec![0.5, -1.0]).convert(DataType::Bool),
        Values::Bool(vec![true, false])
    );
    assert_eq!(
        Values::Float(vec![2.7]).convert(DataType::Int),
        Values::Int(vec![2])
    );
    assert_eq!(
        Values::Vec2(vec![Vec2::new(0.25, 0.5)]).convert(DataType::Color),
        Values::Color(vec![Vec4::new(0.25, 0.5, 0.0, 1.0)])
    );
}

#[test]
fn point_operations_carry_every_point_layer() {
    let mut mesh = cube();
    mesh.attributes.insert(Attribute::new(
        "heat",
        Domain::Point,
        Values::Float((0..8).map(|point| point as f32).collect()),
    ));
    let group = mesh.vertex_groups.add("Arm");
    mesh.vertex_groups.ensure_weights(8);
    mesh.vertex_groups.set_weight(0, group, 1.0);
    mesh.vertex_groups.set_weight(1, group, 0.5);
    let mut lifted = mesh.positions.clone();
    lifted.iter_mut().for_each(|position| position.z += 1.0);
    mesh.shape_keys
        .add(ShapeKey::new("Basis", mesh.positions.clone()));
    mesh.shape_keys.add(ShapeKey::new("Lift", lifted));

    let created = mesh.push_interpolated(Domain::Point, &[(0, 0.5), (1, 0.5)]);
    assert_eq!(created, 8);
    assert_eq!(mesh.positions[8], Vec3::new(0.0, -1.0, -1.0));
    assert_eq!(
        mesh.attributes
            .values::<f32>("heat", Domain::Point)
            .unwrap()[8],
        0.5
    );
    assert_eq!(
        mesh.vertex_groups.weights[8],
        vec![Influence {
            group,
            weight: 0.75
        }]
    );
    assert_eq!(
        mesh.shape_keys.keys[1].positions[8],
        Vec3::new(0.0, -1.0, 0.0)
    );

    mesh.gather(Domain::Point, &[8, 0]);
    assert_eq!(mesh.point_count(), 2);
    assert_eq!(
        mesh.attributes
            .values::<f32>("heat", Domain::Point)
            .unwrap(),
        &[0.5, 0.0]
    );
    assert_eq!(mesh.vertex_groups.weight(1, group), 1.0);
    assert_eq!(mesh.shape_keys.keys[0].positions.len(), 2);
}

#[test]
fn extending_unions_layers_and_converts_types() {
    let sizes = DomainSizes {
        points: 2,
        ..DomainSizes::default()
    };
    let other_sizes = DomainSizes {
        points: 1,
        ..DomainSizes::default()
    };
    let mut mine = Attributes::new();
    mine.insert(Attribute::new(
        "a",
        Domain::Point,
        Values::Float(vec![1.0, 2.0]),
    ));
    mine.insert(Attribute::new(
        "shared",
        Domain::Point,
        Values::Float(vec![0.5, 0.5]),
    ));
    let mut theirs = Attributes::new();
    theirs.insert(Attribute::new("b", Domain::Point, Values::Bool(vec![true])));
    theirs.insert(Attribute::new(
        "shared",
        Domain::Point,
        Values::Int(vec![3]),
    ));
    mine.extend_from(&theirs, sizes, other_sizes);
    assert_eq!(
        mine.values::<f32>("a", Domain::Point).unwrap(),
        &[1.0, 2.0, 0.0]
    );
    assert_eq!(
        mine.values::<bool>("b", Domain::Point).unwrap(),
        &[false, false, true]
    );
    assert_eq!(
        mine.values::<f32>("shared", Domain::Point).unwrap(),
        &[0.5, 0.5, 3.0]
    );
}

#[test]
fn renaming_keeps_uv_maps_and_pins_together() {
    let mut mesh = cube();
    mesh.add_uv_map("UVMap", vec![Vec2::ZERO; mesh.corner_count()]);
    mesh.uv_pins_mut("UVMap")[0] = true;
    assert!(mesh.rename_attribute("UVMap", "Atlas"));
    assert_eq!(mesh.uv_maps.render_name(), Some("Atlas"));
    assert_eq!(mesh.uv_pins("Atlas").map(|pins| pins[0]), Some(true));
    assert!(!mesh.attributes.contains(&uv_pin_name("UVMap")));
    assert!(mesh.remove_uv_map("Atlas"));
    assert!(mesh.attributes.is_empty());
    assert!(mesh.validate().is_ok());
}

#[test]
fn well_known_layers_default_when_absent() {
    let mut mesh = cube();
    assert_eq!(mesh.material_indices(), None);
    assert_eq!(mesh.material_index(3), 0);
    mesh.material_indices_mut()[3] = 2;
    assert_eq!(mesh.material_index(3), 2);
    assert_eq!(mesh.material_indices().map(<[i32]>::len), Some(6));
}

#[test]
fn domains_adapt_like_blender() {
    let mesh = Polymesh::from_polygons(
        crate::support::points(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
        ]),
        [vec![0u32, 1, 2, 3], vec![1, 4, 2]],
    );
    let selected = Values::Bool(vec![true, true, false, false, false, true, true]);
    assert_eq!(
        mesh.adapt_domain(&selected, Domain::Corner, Domain::Point),
        Values::Bool(vec![true, false, false, false, true])
    );
    let heat = Values::Float(vec![0.0, 2.0, 4.0, 6.0, 8.0]);
    let Values::Float(faces) = mesh.adapt_domain(&heat, Domain::Point, Domain::Face) else {
        panic!("float values stay floats");
    };
    assert!(
        (faces[0] - 3.0).abs() < 1.0e-6 && (faces[1] - 14.0 / 3.0).abs() < 1.0e-6,
        "{faces:?}"
    );
    let uvs = vec2(&[
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
    ]);
    let Values::Vec2(faces) = mesh.adapt_domain(&Values::Vec2(uvs), Domain::Corner, Domain::Face)
    else {
        panic!("vectors stay vectors");
    };
    assert!(faces[0].abs_diff_eq(Vec2::splat(0.5), 1.0e-6));
    assert!(faces[1].abs_diff_eq(Vec2::new(2.0 / 3.0, 1.0 / 3.0), 1.0e-6));
}
