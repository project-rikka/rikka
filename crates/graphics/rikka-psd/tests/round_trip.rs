use rikka_psd::{
    Adjustment, BlendMode, BlendRange, BlendingRanges, ChannelId, Color, ColorMode, Compression,
    Depth, Descriptor, Document, Effects, GradientFill, Group, Id, Info, Key, Knockout, Knot,
    Layer, LayerMask, LevelRecord, Levels, MaskParameters, Node, Overlay, PathData, PathOperation,
    Plane, Point, RealMask, Rect, Resolution, ResourceData, SectionKind, Shadow, SheetColor,
    Stroke, Subpath, Unit, Value, VectorMask, Version, resource_id,
};

fn gradient_plane(width: u32, height: u32, depth: Depth, seed: u32) -> Plane {
    let values: Vec<f32> = (0..width * height)
        .map(|index| ((index * 7 + seed * 13) % 97) as f32 / 96.0)
        .collect();
    Plane::from_normalized(width, height, depth, &values).unwrap()
}

fn pixel_layer(name: &str, bounds: Rect, depth: Depth, compression: Compression) -> Layer {
    let mut layer = Layer::new(name);
    let planes: Vec<(ChannelId, Plane)> = [
        ChannelId::Transparency,
        ChannelId::Color(0),
        ChannelId::Color(1),
        ChannelId::Color(2),
    ]
    .into_iter()
    .enumerate()
    .map(|(seed, id)| {
        (
            id,
            gradient_plane(bounds.width(), bounds.height(), depth, seed as u32),
        )
    })
    .collect();
    layer.set_pixels(bounds, &planes, compression).unwrap();
    layer
}

fn rich_document(version: Version, depth: Depth) -> Document {
    let mut document = Document::new(64, 48, depth, ColorMode::Rgb);
    document.version = version;
    document.resources.set(
        resource_id::RESOLUTION,
        ResourceData::Resolution(Resolution::pixels_per_inch(144.0)),
    );
    document
        .resources
        .set(resource_id::GLOBAL_ANGLE, ResourceData::GlobalAngle(90));
    document.resources.set(
        resource_id::XMP,
        ResourceData::Xmp("<x:xmpmeta/>".to_owned()),
    );
    document.resources.set(
        resource_id::LAYER_SELECTION,
        ResourceData::LayerSelection(vec![3, 4]),
    );
    document.resources.set(
        resource_id::PRINT_INFO,
        ResourceData::Descriptor {
            version: None,
            descriptor: Descriptor::new("printOutput").with("PstS", true),
        },
    );

    let mut base = pixel_layer(
        "Background ✓",
        Rect::new(0, 0, 64, 48),
        depth,
        Compression::Rle,
    );
    base.blending_ranges = BlendingRanges {
        gray: Some(BlendRange {
            source: [10, 20, 230, 245],
            destination: [0, 0, 255, 255],
        }),
        channels: vec![BlendRange::FULL; 3],
    };
    base.set_info(Info::Id(3));
    base.set_info(Info::SheetColor(SheetColor::Violet));

    let mut masked = pixel_layer(
        "Masked",
        Rect::new(5, 6, 30, 26),
        depth,
        Compression::ZipPredicted,
    );
    let mask_bounds = Rect::new(4, 4, 20, 20);
    let mut mask = LayerMask::new(mask_bounds, 255);
    mask.parameters = MaskParameters {
        user_density: Some(200),
        user_feather: Some(2.5),
        vector_density: None,
        vector_feather: Some(1.0),
    };
    mask.real = Some(RealMask {
        flags: Default::default(),
        default_color: 0,
        bounds: Rect::new(0, 0, 8, 8),
    });
    masked
        .set_mask(mask, &gradient_plane(16, 16, depth, 9), Compression::Zip)
        .unwrap();
    masked
        .set_channel(
            ChannelId::RealUserMask,
            &gradient_plane(8, 8, depth, 11),
            Compression::Raw,
        )
        .unwrap();
    masked.blend_mode = BlendMode::Multiply;
    masked.opacity = 128;
    masked.set_fill_opacity(0.5);
    masked.set_info(Info::Knockout(Knockout::Deep));
    let mut effects = Effects::new();
    effects.drop_shadows.push(Shadow::new(Color::BLACK));
    effects
        .strokes
        .push(Stroke::new(3.0, Color::rgb8(255, 0, 0)));
    effects
        .gradient_overlays
        .push(Overlay::gradient(GradientFill::new(
            rikka_psd::Gradient::two_color(Color::WHITE, Color::BLACK),
        )));
    masked.set_effects(Some(&effects));
    masked.set_info(Info::VectorMask(VectorMask::new(PathData::new(vec![
        Subpath::new(
            true,
            PathOperation::Combine,
            vec![
                Knot::corner(Point { x: 0.125, y: 0.125 }),
                Knot::corner(Point { x: 0.875, y: 0.125 }),
                Knot::corner(Point { x: 0.5, y: 0.875 }),
            ],
        ),
    ]))));

    let mut clipped = pixel_layer(
        "Clipped",
        Rect::new(10, 10, 20, 20),
        depth,
        Compression::Raw,
    );
    clipped.clipping = true;
    clipped.set_visible(false);

    let mut adjustment = Layer::new("Levels");
    adjustment.set_info(Info::Adjustment(Adjustment::Levels(Levels {
        version: 2,
        records: vec![
            LevelRecord {
                input_floor: 10,
                input_ceiling: 240,
                output_floor: 0,
                output_ceiling: 255,
                gamma: 120,
            };
            29
        ],
        extra_version: None,
    })));

    let mut group = Group::new("Group");
    group.set_blend_mode(BlendMode::Screen);
    group.children = vec![Node::Layer(clipped.clone()), Node::Layer(adjustment)];
    let mut nested = Group::new("Nested");
    nested.set_open(false);
    nested.children = vec![Node::Layer(pixel_layer(
        "Deep",
        Rect::new(1, 1, 3, 3),
        depth,
        Compression::Rle,
    ))];
    group.children.push(Node::Group(nested));

    document.layers = vec![Node::Layer(base), Node::Layer(masked), Node::Group(group)];
    document.set_info(Info::Raw {
        key: Key(*b"Xtra"),
        data: vec![1, 2, 3, 4],
    });
    document.set_info(Info::descriptor(
        Key(*b"artd"),
        Descriptor::new("null").with("Cnt ", 1).with(
            "origin",
            Value::Descriptor(Descriptor::new("Pnt ").with("Hrzn", 0.0).with("Vrtc", 0.0)),
        ),
    ));
    let composite: Vec<Plane> = (0..3)
        .map(|seed| gradient_plane(64, 48, depth, seed))
        .collect();
    document
        .set_composite(&composite, Compression::Rle)
        .unwrap();
    document
}

#[test]
fn documents_round_trip() {
    for version in [Version::Psd, Version::Psb] {
        for depth in [Depth::Eight, Depth::Sixteen, Depth::ThirtyTwo] {
            let document = rich_document(version, depth);
            let bytes = document.to_bytes().unwrap();
            let read = Document::read(&bytes).unwrap();
            assert_eq!(read.resources, document.resources, "{version:?} {depth:?}");
            for (got, want) in read
                .layer_records()
                .into_iter()
                .zip(document.layer_records())
            {
                assert_eq!(got.info, want.info, "{version:?} {depth:?} {}", want.name());
                assert_eq!(got, want, "{version:?} {depth:?} {}", want.name());
            }
            assert_eq!(read.info, document.info, "{version:?} {depth:?}");
            assert_eq!(read, document, "{version:?} {depth:?}");
            assert_eq!(
                read.to_bytes().unwrap(),
                bytes,
                "{version:?} {depth:?} is stable"
            );
        }
    }
}

#[test]
fn layer_tree_keeps_order_and_groups() {
    let document = Document::read(
        &rich_document(Version::Psd, Depth::Eight)
            .to_bytes()
            .unwrap(),
    )
    .unwrap();
    let names: Vec<_> = document.layers.iter().map(Node::name).collect();
    assert_eq!(names, ["Background ✓", "Masked", "Group"]);
    let group = document.layers[2].as_group().unwrap();
    assert_eq!(group.blend_mode(), BlendMode::Screen);
    assert!(group.is_open());
    let nested = group.children[2].as_group().unwrap();
    assert!(!nested.is_open());
    assert_eq!(
        nested.end.section().unwrap().kind,
        SectionKind::BoundingDivider
    );
    assert_eq!(document.layer_records().len(), 9);
}

#[test]
fn channels_decode_to_encoded_values() {
    let depth = Depth::Sixteen;
    let document = Document::read(&rich_document(Version::Psb, depth).to_bytes().unwrap()).unwrap();
    let masked = document.layers[1].layer();
    let expected = gradient_plane(25, 20, depth, 1);
    assert_eq!(
        masked
            .decode_channel(ChannelId::Color(0), depth)
            .unwrap()
            .unwrap(),
        expected
    );
    assert_eq!(
        masked
            .decode_channel(ChannelId::UserMask, depth)
            .unwrap()
            .unwrap(),
        gradient_plane(16, 16, depth, 9)
    );
    assert_eq!(
        masked
            .decode_channel(ChannelId::RealUserMask, depth)
            .unwrap()
            .unwrap(),
        gradient_plane(8, 8, depth, 11)
    );
}

#[test]
fn styles_round_trip() {
    let document = Document::read(
        &rich_document(Version::Psd, Depth::Eight)
            .to_bytes()
            .unwrap(),
    )
    .unwrap();
    let masked = document.layers[1].layer();
    let effects = masked.effects().unwrap();
    assert_eq!(effects.drop_shadows.len(), 1);
    assert_eq!(
        effects.drop_shadows[0].common.blend_mode,
        BlendMode::Multiply
    );
    assert!((effects.drop_shadows[0].common.opacity - 0.75).abs() < 1e-9);
    assert_eq!(effects.strokes[0].size, 3.0);
    assert_eq!(effects.gradient_overlays.len(), 1);
    assert_eq!(masked.knockout(), Knockout::Deep);
    assert!((masked.fill_opacity() - 128.0 / 255.0).abs() < 0.01);
    let path = masked.vector_mask().unwrap();
    assert_eq!(path.path.subpaths().next().unwrap().knots.len(), 3);
    let artd = document.find(b"artd").unwrap();
    let Info::Descriptor(block) = artd else {
        panic!("artd is descriptor")
    };
    assert_eq!(block.descriptor.integer("Cnt "), Some(1));
}

#[test]
fn unknown_blocks_are_kept_verbatim() {
    let document = Document::read(
        &rich_document(Version::Psd, Depth::Eight)
            .to_bytes()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        document.find(b"Xtra"),
        Some(&Info::Raw {
            key: Key(*b"Xtra"),
            data: vec![1, 2, 3, 4],
        })
    );
}

#[test]
fn every_descriptor_value_round_trips() {
    let inner = Descriptor::new("Pnt ").with("Hrzn", Value::UnitDouble(Unit::PIXELS, 3.5));
    let descriptor = Descriptor::new("null")
        .with("Objc", Value::Descriptor(inner.clone()))
        .with("GlbO", Value::GlobalObject(inner.clone()))
        .with(
            "list",
            Value::List(vec![Value::Integer(1), Value::String("two".into())]),
        )
        .with("doub", 1.25)
        .with("UnFl", Value::UnitFloats(Unit::PIXELS, vec![1.0, 2.0]))
        .with("TEXT", "テキスト")
        .with(
            "enum",
            Value::Enum(Id::from("BlnM"), Id::from("linearBurn")),
        )
        .with("comp", Value::LargeInteger(-5_000_000_000))
        .with("bool", true)
        .with("type", Value::Class(rikka_psd::Class::new("Lyr ")))
        .with("tdta", Value::RawData(vec![0, 1, 2]))
        .with("alis", Value::Alias(vec![9, 9]))
        .with(
            "obj ",
            Value::Reference(vec![
                rikka_psd::Reference::Enumerated {
                    class: rikka_psd::Class::new("Lyr "),
                    kind: Id::from("Ordn"),
                    value: Id::from("Trgt"),
                },
                rikka_psd::Reference::Name {
                    class: rikka_psd::Class::new("Lyr "),
                    value: "Layer".into(),
                },
                rikka_psd::Reference::Index(4),
                rikka_psd::Reference::Identifier(7),
            ]),
        )
        .with(
            "ObAr",
            Value::ObjectArray(rikka_psd::ObjectArray {
                count: 2,
                name: String::new(),
                class: Id::from("rationalPoint"),
                items: vec![(
                    Id::from("Hrzn"),
                    Value::UnitFloats(Unit::PIXELS, vec![1.0, 2.0]),
                )],
            }),
        );
    let mut document = Document::new(4, 4, Depth::Eight, ColorMode::Rgb);
    document.set_info(Info::descriptor(Key(*b"cinf"), descriptor.clone()));
    let read = Document::read(&document.to_bytes().unwrap()).unwrap();
    let Some(Info::Descriptor(block)) = read.find(b"cinf") else {
        panic!("cinf block survives")
    };
    assert_eq!(block.descriptor, descriptor);
}

#[test]
fn oversized_documents_require_psb() {
    let mut document = Document::new(40_000, 10, Depth::Eight, ColorMode::Grayscale);
    assert_eq!(document.version, Version::Psb);
    document.version = Version::Psd;
    assert!(document.to_bytes().is_err());
}
