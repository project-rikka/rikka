use glam::{Vec2, Vec3};
use rikka_file_format::{Codec, DecodeError, Reader, Writer};
use rikka_mesh_modifiers::{
    Armature, Array, Bevel, Boolean, CornerData, CornerMapping, DataTransfer, Decimate, Delimit,
    Displace, DisplaceDirection, EdgeSplit, LayerSelection, LayersDestination, LayersSource,
    Mirror, MixMode, Modifier, ModifierKind, ObjectRef, Shrinkwrap, SmoothByAngle, Solidify,
    SolidifyMode, Subdivision, TextureRef, Triangulate, VertexData, Weld, WeldMode, WrapMethod,
};

fn encoded<T: Codec>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value.encode(&mut Writer::new(&mut bytes));
    bytes
}

fn decoded<T: Codec<Error = DecodeError>>(bytes: &[u8]) -> Result<T, DecodeError> {
    let mut reader = Reader::new(bytes);
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

fn stack() -> Vec<Modifier> {
    let mut mirror = Mirror {
        use_axis: [true, false, true],
        use_bisect_axis: [false, false, true],
        mirror_object: Some(ObjectRef::new(7)),
        mirror_offset: Vec2::new(0.25, -0.5),
        ..Mirror::default()
    };
    mirror.use_mirror_u = true;
    let transfer = DataTransfer {
        object: Some(ObjectRef::new(3)),
        use_vert_data: true,
        data_types_verts: VertexData::VERTEX_GROUPS | VertexData::COLOR,
        use_loop_data: true,
        data_types_loops: CornerData::UV,
        loop_mapping: CornerMapping::FaceInterpolatedNormalProjected,
        layers_uv: LayerSelection {
            source: LayersSource::Layer(2),
            destination: LayersDestination::Layer(1),
        },
        mix_mode: MixMode::Subtract,
        mix_factor: 0.25,
        ..DataTransfer::default()
    };
    let mut hidden = Modifier::new(
        "Hidden",
        Weld {
            mode: WeldMode::Connected,
            ..Weld::default()
        },
    );
    hidden.show_render = false;
    hidden.show_on_cage = true;
    vec![
        Modifier::new("Mirror", mirror),
        Modifier::new(
            "Subdivision",
            Subdivision {
                levels: 3,
                render_levels: 4,
                ..Subdivision::default()
            },
        ),
        Modifier::new(
            "Solidify",
            Solidify {
                solidify_mode: SolidifyMode::Complex,
                thickness: -0.2,
                material_offset: -3,
                ..Solidify::default()
            },
        ),
        Modifier::new(
            "Array",
            Array {
                count: 5,
                constant_offset_displace: Vec3::new(1.0, 2.0, 3.0),
                start_cap: Some(ObjectRef::new(11)),
                ..Array::default()
            },
        ),
        hidden,
        Modifier::new("Triangulate", Triangulate::default()),
        Modifier::new("EdgeSplit", EdgeSplit::default()),
        Modifier::new(
            "Displace",
            Displace {
                texture: Some(TextureRef::new(4)),
                direction: DisplaceDirection::RgbToXyz,
                ..Displace::default()
            },
        ),
        Modifier::new(
            "Shrinkwrap",
            Shrinkwrap {
                wrap_method: WrapMethod::TargetProject,
                target: Some(ObjectRef::new(9)),
                ..Shrinkwrap::default()
            },
        ),
        Modifier::new("DataTransfer", transfer),
        Modifier::new("Smooth by Angle", SmoothByAngle::default()),
        Modifier::new(
            "Armature",
            Armature {
                object: Some(ObjectRef::new(1)),
                ..Armature::default()
            },
        ),
        Modifier::new("Bevel", Bevel::default()),
        Modifier::new("Boolean", Boolean::default()),
        Modifier::new(
            "Decimate",
            Decimate {
                delimit: Delimit::SEAM | Delimit::UV,
                ..Decimate::default()
            },
        ),
    ]
}

#[test]
fn stack_round_trips() {
    let stack = stack();
    let bytes = encoded(&stack);
    let restored: Vec<Modifier> = decoded(&bytes).expect("the encoded stack decodes");
    assert_eq!(restored, stack);
}

#[test]
fn truncated_input_is_refused() {
    let bytes = encoded(&stack());
    for length in [0, 1, 9, bytes.len() / 2, bytes.len() - 1] {
        assert!(
            decoded::<Vec<Modifier>>(&bytes[..length]).is_err(),
            "{length}"
        );
    }
}

type Corruption = Box<dyn Fn(&mut Modifier)>;

#[test]
fn hostile_values_are_refused() {
    let cases: Vec<(Modifier, Corruption)> = vec![
        (
            Modifier::new("Subdivision", Subdivision::default()),
            Box::new(|modifier| {
                if let ModifierKind::Subdivision(subdivision) = &mut modifier.kind {
                    subdivision.levels = 12;
                }
            }),
        ),
        (
            Modifier::new("Weld", Weld::default()),
            Box::new(|modifier| {
                if let ModifierKind::Weld(weld) = &mut modifier.kind {
                    weld.merge_threshold = f32::NAN;
                }
            }),
        ),
        (
            Modifier::new("Array", Array::default()),
            Box::new(|modifier| {
                if let ModifierKind::Array(array) = &mut modifier.kind {
                    array.count = 0;
                }
            }),
        ),
        (
            Modifier::new("Triangulate", Triangulate::default()),
            Box::new(|modifier| {
                if let ModifierKind::Triangulate(triangulate) = &mut modifier.kind {
                    triangulate.min_vertices = 3;
                }
            }),
        ),
        (
            Modifier::new("DataTransfer", DataTransfer::default()),
            Box::new(|modifier| {
                if let ModifierKind::DataTransfer(transfer) = &mut modifier.kind {
                    transfer.mix_factor = 1.5;
                }
            }),
        ),
    ];
    for (mut modifier, corrupt) in cases {
        corrupt(&mut modifier);
        let bytes = encoded(&modifier);
        assert!(decoded::<Modifier>(&bytes).is_err(), "{}", modifier.name);
    }
}

#[test]
fn unknown_tags_are_refused() {
    let mut bytes = encoded(&Modifier::new("Weld", Weld::default()));
    let name_end = 4 + "Weld".len() + 5;
    bytes[name_end] = 200;
    assert!(decoded::<Modifier>(&bytes).is_err());
}
