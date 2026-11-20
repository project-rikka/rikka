use rikka_psd::{
    Affine, Axis, ChannelId, Color, ColorMode, Compression, CpuResampler, Depth, Descriptor,
    Document, Effects, Geometry, GridAndGuides, Guide, GuideDirection, Info, Kernel, Knot, Layer,
    LayerMask, Node, PathData, PathOperation, PlacedKind, PlacedLayer, Plane, Point, QuarterTurn,
    Rect, ResampleOptions, Resolution, ResourceData, Shadow, Subpath, TypeTool, Value, VectorMask,
    resource_id,
};

use crate::support::{numbered, reds};

fn ink(document: &Document) -> &Layer {
    let Node::Layer(layer) = &document.layers[0] else {
        panic!("expected layer");
    };
    layer
}

fn knot(document: &Document) -> Point {
    let Some(Info::VectorMask(mask)) = ink(document).find(b"vmsk") else {
        panic!("expected vector mask");
    };
    mask.path.subpaths().next().unwrap().knots[0].anchor
}

fn text_transform(document: &Document) -> [f64; 6] {
    let Some(Info::TypeTool(tool)) = ink(document).find(b"TySh") else {
        panic!("expected text layer");
    };
    tool.transform
}

fn placed_corners(document: &Document) -> [f64; 8] {
    let Some(Info::PlacedLayer(placed)) = ink(document).find(b"PlLd") else {
        panic!("expected placed layer");
    };
    placed.transform
}

fn smart_object_corners(document: &Document) -> Vec<f64> {
    ink(document)
        .descriptor(&[b"SoLd"])
        .and_then(|descriptor| descriptor.list("Trnf"))
        .unwrap()
        .iter()
        .filter_map(Value::number)
        .collect()
}

fn guides(document: &Document) -> Vec<Guide> {
    let Some(ResourceData::GridAndGuides(grid)) =
        document.resources.get(resource_id::GRID_AND_GUIDES)
    else {
        panic!("expected guides");
    };
    grid.guides.clone()
}

fn mask_values(document: &Document) -> Vec<u8> {
    document
        .layer_mask_plane(ink(document))
        .unwrap()
        .unwrap()
        .to_normalized()
        .iter()
        .map(|value| (value * 255.0).round() as u8)
        .collect()
}

fn layered() -> Document {
    let mut document = Document::new(4, 3, Depth::Eight, ColorMode::Rgb);
    document
        .set_composite_rgba(&numbered(4, 3), Compression::Rle)
        .unwrap();
    let mut layer = Layer::new("ink");
    document
        .set_layer_rgba(&mut layer, 0, 1, &numbered(2, 2), Compression::Zip)
        .unwrap();
    let mask = Plane::from_normalized(2, 2, Depth::Eight, &[0.0, 0.2, 0.6, 1.0]).unwrap();
    layer
        .set_mask(
            LayerMask::new(Rect::from_size(0, 1, 2, 2), 0),
            &mask,
            Compression::Rle,
        )
        .unwrap();
    let corner = Knot::corner(Point {
        x: 0.25,
        y: 1.0 / 3.0,
    });
    layer.set_info(Info::VectorMask(VectorMask::new(PathData::new(vec![
        Subpath::new(true, PathOperation::Combine, vec![corner]),
    ]))));
    layer.set_info(Info::TypeTool(TypeTool {
        version: 1,
        transform: [1.0, 0.0, 0.0, 1.0, 3.0, 2.0],
        text_version: 50,
        text: Descriptor::new("TxLr"),
        warp_version: 1,
        warp: Descriptor::new("warp"),
        bounds: [0.0; 4],
    }));
    layer.set_info(Info::PlacedLayer(PlacedLayer {
        version: 3,
        unique_id: "placed".into(),
        page: 1,
        total_pages: 1,
        anti_alias: 16,
        kind: PlacedKind::Raster,
        transform: [0.0, 0.0, 2.0, 0.0, 2.0, 1.0, 0.0, 1.0],
        warp_version: 0,
        warp: Descriptor::new("warp"),
        legacy_key: false,
    }));
    let corners: Vec<Value> = [0.0, 0.0, 2.0, 0.0, 2.0, 1.0, 0.0, 1.0]
        .into_iter()
        .map(Value::Double)
        .collect();
    layer.set_info(Info::descriptor(
        rikka_psd::Key(*b"SoLd"),
        Descriptor::new("null").with("Trnf", Value::List(corners)),
    ));
    layer.set_info(Info::ReferencePoint { x: 1.0, y: 2.0 });
    document.layers.push(Node::Layer(layer));
    document.layers.push(Node::Group(
        rikka_psd::Group::new("folder").with_children(vec![Node::Layer(Layer::new("empty"))]),
    ));
    document.resources.set(
        resource_id::GRID_AND_GUIDES,
        ResourceData::GridAndGuides(GridAndGuides {
            version: 1,
            grid_horizontal: 576,
            grid_vertical: 576,
            guides: vec![
                Guide {
                    position: 1.0,
                    direction: GuideDirection::Vertical,
                },
                Guide {
                    position: 2.0,
                    direction: GuideDirection::Horizontal,
                },
            ],
        }),
    );
    document.resources.set(
        resource_id::RESOLUTION,
        ResourceData::Resolution(Resolution {
            vertical: 144.0,
            ..Resolution::pixels_per_inch(72.0)
        }),
    );
    document
}

fn reread(document: &Document) -> Document {
    Document::read(&document.to_bytes().unwrap()).unwrap()
}

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-9, "{actual:?} vs {expected:?}");
    }
}

#[test]
fn flip_mirrors_pixels_masks_and_records() {
    let mut document = layered();
    document
        .resources
        .set(resource_id::THUMBNAIL, ResourceData::Raw(vec![1, 2, 3]));

    document.flip(Axis::Horizontal).unwrap();

    assert_eq!((document.width, document.height), (4, 3));
    assert_eq!(
        reds(&document.composite_rgba().unwrap()),
        [3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8]
    );
    let layer = ink(&document);
    assert_eq!(layer.bounds, Rect::from_size(2, 1, 2, 2));
    assert_eq!(layer.mask.unwrap().bounds, Rect::from_size(2, 1, 2, 2));
    assert_eq!(reds(&document.layer_rgba(layer).unwrap()), [1, 0, 3, 2]);
    assert_eq!(mask_values(&document), [51, 0, 255, 153]);
    assert_eq!(
        layer
            .channel(ChannelId::Color(0))
            .unwrap()
            .data
            .compression(),
        Compression::Zip
    );
    close(&[knot(&document).x, knot(&document).y], &[0.75, 1.0 / 3.0]);
    close(&text_transform(&document), &[-1.0, 0.0, 0.0, 1.0, 1.0, 2.0]);
    close(
        &placed_corners(&document),
        &[4.0, 0.0, 2.0, 0.0, 2.0, 1.0, 4.0, 1.0],
    );
    close(
        &smart_object_corners(&document),
        &[4.0, 0.0, 2.0, 0.0, 2.0, 1.0, 4.0, 1.0],
    );
    assert_eq!(
        layer.find(b"fxrp"),
        Some(&Info::ReferencePoint { x: 3.0, y: 2.0 })
    );
    assert_eq!(
        guides(&document)
            .iter()
            .map(|guide| guide.position)
            .collect::<Vec<_>>(),
        [3.0, 2.0]
    );
    assert!(document.resources.get(resource_id::THUMBNAIL).is_none());
    assert_eq!(reread(&document).layers.len(), 2);
}

#[test]
fn clockwise_turn_swaps_canvas_and_rotates_records() {
    let mut document = layered();

    document.rotate_quarter(QuarterTurn::Clockwise).unwrap();

    assert_eq!((document.width, document.height), (3, 4));
    let mut expected = vec![0u8; 12];
    for y in 0..3 {
        for x in 0..4 {
            expected[x * 3 + (2 - y)] = (y * 4 + x) as u8;
        }
    }
    assert_eq!(reds(&document.composite_rgba().unwrap()), expected);
    let layer = ink(&document);
    assert_eq!(layer.bounds, Rect::from_size(0, 0, 2, 2));
    assert_eq!(reds(&document.layer_rgba(layer).unwrap()), [2, 0, 3, 1]);
    assert_eq!(mask_values(&document), [153, 0, 255, 51]);
    close(&text_transform(&document), &[0.0, 1.0, -1.0, 0.0, 1.0, 3.0]);
    close(
        &placed_corners(&document),
        &[3.0, 0.0, 3.0, 2.0, 2.0, 2.0, 2.0, 0.0],
    );
    close(&[knot(&document).x, knot(&document).y], &[2.0 / 3.0, 0.25]);
    let guides = guides(&document);
    assert_eq!(guides[0].direction, GuideDirection::Horizontal);
    assert_eq!(guides[0].position, 1.0);
    assert_eq!(guides[1].direction, GuideDirection::Vertical);
    assert_eq!(guides[1].position, 1.0);
    let Some(resolution) = document.resources.resolution() else {
        panic!("expected resolution");
    };
    assert_eq!((resolution.horizontal, resolution.vertical), (144.0, 72.0));
    assert_eq!(reread(&document).width, 3);
}

#[test]
fn double_flip_and_full_turn_are_identity() {
    let original = layered();
    let mut document = original.clone();

    document.flip(Axis::Horizontal).unwrap();
    document.flip(Axis::Horizontal).unwrap();
    document.flip(Axis::Vertical).unwrap();
    document.flip(Axis::Vertical).unwrap();
    assert_eq!(document, original);

    for _ in 0..4 {
        document.rotate_quarter(QuarterTurn::Clockwise).unwrap();
    }
    assert_eq!(document, original);

    document.rotate_quarter(QuarterTurn::Half).unwrap();
    document
        .rotate_quarter(QuarterTurn::CounterClockwise)
        .unwrap();
    document
        .rotate_quarter(QuarterTurn::CounterClockwise)
        .unwrap();
    assert_eq!(document, original);
}

fn with_background() -> Document {
    let mut document = Document::new(2, 2, Depth::Eight, ColorMode::Rgb);
    document
        .set_composite_rgba(&numbered(2, 2), Compression::Rle)
        .unwrap();
    let mut background = Layer::new("Background");
    let planes: Vec<(ChannelId, Plane)> = (0..3)
        .map(|channel| {
            let values = [0.0, 0.25, 0.5, 1.0].map(|value| value * (channel + 1) as f32 / 3.0);
            (
                ChannelId::Color(channel),
                Plane::from_normalized(2, 2, Depth::Eight, &values).unwrap(),
            )
        })
        .collect();
    background
        .set_pixels(Rect::from_size(0, 0, 2, 2), &planes, Compression::Rle)
        .unwrap();
    document.layers.push(Node::Layer(background));
    document
}

#[test]
fn canvas_grow_then_shrink_is_identity() {
    for original in [layered(), with_background()] {
        let mut document = original.clone();
        let (width, height) = (document.width, document.height);

        document
            .resize_canvas(Rect::from_size(-2, -1, width + 5, height + 3), Color::BLACK)
            .unwrap();
        document
            .resize_canvas(Rect::from_size(2, 1, width, height), Color::BLACK)
            .unwrap();

        assert_eq!(document, original);
    }
}

#[test]
fn resample_2x_scales_pixels_records_and_styles() {
    let mut document = layered();
    let mut effects = Effects::new();
    effects.drop_shadows.push(Shadow::new(Color::BLACK));
    let Node::Layer(layer) = &mut document.layers[0] else {
        panic!("expected layer");
    };
    layer.set_effects(Some(&effects));
    let options = ResampleOptions {
        resolution: Some(Resolution::pixels_per_inch(300.0)),
        ..ResampleOptions::default()
    };

    document
        .resample(
            Geometry::resize((4, 3), 8, 6),
            &options,
            &mut CpuResampler::new(Kernel::Bilinear),
        )
        .unwrap();

    assert_eq!((document.width, document.height), (8, 6));
    let layer = ink(&document);
    assert_eq!(layer.bounds, Rect::from_size(0, 2, 4, 4));
    assert_eq!(layer.mask.unwrap().bounds, Rect::from_size(0, 2, 4, 4));
    let alpha = document
        .layer_rgba(layer)
        .unwrap()
        .pixels
        .iter()
        .map(|pixel| pixel[3])
        .collect::<Vec<_>>();
    assert!(alpha.iter().all(|&value| value > 0.0));
    close(&text_transform(&document), &[2.0, 0.0, 0.0, 2.0, 6.0, 4.0]);
    close(&[knot(&document).x, knot(&document).y], &[0.25, 1.0 / 3.0]);
    close(
        &placed_corners(&document),
        &[0.0, 0.0, 4.0, 0.0, 4.0, 2.0, 0.0, 2.0],
    );
    assert_eq!(
        guides(&document)
            .iter()
            .map(|guide| guide.position)
            .collect::<Vec<_>>(),
        [2.0, 4.0]
    );
    let shadow = &layer.effects().unwrap().drop_shadows[0];
    assert_eq!((shadow.distance, shadow.size), (10.0, 10.0));
    assert_eq!(
        document.resources.resolution(),
        Some(&Resolution::pixels_per_inch(300.0))
    );
    let reds = reds(&document.composite_rgba().unwrap());
    assert_eq!((reds[0], reds[47]), (0, 11));
    assert_eq!(reread(&document).width, 8);
}

#[test]
fn right_angle_resample_matches_exact_turn() {
    let mut exact = layered();
    exact.rotate_quarter(QuarterTurn::Clockwise).unwrap();
    let mut resampled = layered();

    resampled
        .resample(
            Geometry::rotate((4, 3), 90.0),
            &ResampleOptions::default(),
            &mut CpuResampler::new(Kernel::Nearest),
        )
        .unwrap();

    assert_eq!((resampled.width, resampled.height), (3, 4));
    assert_eq!(
        resampled.composite_planes().unwrap(),
        exact.composite_planes().unwrap()
    );
    assert_eq!(ink(&resampled).bounds, ink(&exact).bounds);
    assert_eq!(
        resampled.layer_rgba(ink(&resampled)).unwrap(),
        exact.layer_rgba(ink(&exact)).unwrap()
    );
    let guides = guides(&resampled);
    assert_eq!(guides[0].direction, GuideDirection::Horizontal);
    assert!((guides[0].position - 1.0).abs() < 1e-9);
}

#[test]
fn invalid_geometry_leaves_document_unchanged() {
    let mut document = layered();

    assert!(
        document
            .resample(
                Geometry::new(4, 3, Affine::scale(0.0, 1.0)),
                &ResampleOptions::default(),
                &mut CpuResampler::default(),
            )
            .is_err()
    );
    assert!(
        document
            .resize_canvas(Rect::from_size(0, 0, 0, 3), Color::WHITE)
            .is_err()
    );

    assert_eq!(document, layered());
}

#[test]
fn flip_moves_live_shapes_artboards_and_slices() {
    let mut document = layered();
    let point = |x: f64, y: f64| Descriptor::new("Pnt ").with("Hrzn", x).with("Vrtc", y);
    let identity = Descriptor::new("Trnf")
        .with("xx", 1.0)
        .with("xy", 0.0)
        .with("yx", 0.0)
        .with("yy", 1.0)
        .with("tx", 0.0)
        .with("ty", 0.0);
    let origination = Descriptor::new("null").with("Trnf", identity).with(
        "keyOriginBoxCorners",
        Descriptor::new("null").with("rectangleCornerA", point(1.0, 0.0)),
    );
    let rect = |left: i32, right: i32| {
        Descriptor::new("Rct1")
            .with("Top ", 0)
            .with("Left", left)
            .with("Btom", 3)
            .with("Rght", right)
    };
    let Node::Layer(layer) = &mut document.layers[0] else {
        panic!("expected layer");
    };
    layer.set_info(Info::descriptor(
        rikka_psd::Key(*b"vogk"),
        Descriptor::new("null").with(
            "keyDescriptorList",
            Value::List(vec![Value::Descriptor(origination)]),
        ),
    ));
    layer.set_info(Info::descriptor(
        rikka_psd::Key(*b"artb"),
        Descriptor::new("artboard").with("artboardRect", rect(0, 2)),
    ));
    document.resources.set(
        resource_id::SLICES,
        ResourceData::Descriptor {
            version: Some(7),
            descriptor: Descriptor::new("null").with("bounds", rect(0, 1)),
        },
    );

    document.flip(Axis::Horizontal).unwrap();

    let layer = ink(&document);
    let origination = layer
        .descriptor(&[b"vogk"])
        .unwrap()
        .list("keyDescriptorList")
        .unwrap()[0]
        .as_descriptor()
        .unwrap();
    let transform = origination.descriptor("Trnf").unwrap();
    close(
        &["xx", "xy", "yx", "yy", "tx", "ty"].map(|key| transform.number(key).unwrap()),
        &[-1.0, 0.0, 0.0, 1.0, 4.0, 0.0],
    );
    let corner = origination
        .descriptor("keyOriginBoxCorners")
        .and_then(|corners| corners.descriptor("rectangleCornerA"))
        .unwrap();
    close(
        &[
            corner.number("Hrzn").unwrap(),
            corner.number("Vrtc").unwrap(),
        ],
        &[3.0, 0.0],
    );
    let artboard = layer
        .descriptor(&[b"artb"])
        .and_then(|descriptor| descriptor.descriptor("artboardRect"))
        .unwrap();
    assert_eq!(
        (artboard.integer("Left"), artboard.integer("Rght")),
        (Some(2), Some(4))
    );
    let Some(ResourceData::Descriptor { descriptor, .. }) =
        document.resources.get(resource_id::SLICES)
    else {
        panic!("expected slices");
    };
    let bounds = descriptor.descriptor("bounds").unwrap();
    assert_eq!(bounds.get("Left"), Some(&Value::Integer(3)));
    assert_eq!(bounds.get("Rght"), Some(&Value::Integer(4)));
    assert_eq!(reread(&document).layers.len(), 2);
}
