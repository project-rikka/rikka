use rikka_psd::{
    Adjustment, BlendMode, ChannelId, Color, ColorMode, Compression, Depth, Document, Effects,
    FillContent, Group, Info, Knockout, Knot, Layer, LayerMask, Node, Overlay, PathData,
    PathOperation, Plane, Point, Rect, RgbaImage, Shadow, Stroke, Subpath, VectorMask,
};
use rikka_psd_render::{Renderer, render_rgba};

fn solid(name: &str, bounds: Rect, rgba: [u8; 4]) -> Layer {
    let document = Document::new(1, 1, Depth::Eight, ColorMode::Rgb);
    let mut layer = Layer::new(name);
    let pixels: Vec<u8> = (0..bounds.area()).flat_map(|_| rgba).collect();
    let image = RgbaImage::from_rgba8(bounds.width(), bounds.height(), &pixels).unwrap();
    document
        .set_layer_rgba(
            &mut layer,
            bounds.left,
            bounds.top,
            &image,
            Compression::Rle,
        )
        .unwrap();
    layer
}

fn document(width: u32, height: u32, layers: Vec<Node>) -> Document {
    let mut document = Document::new(width, height, Depth::Eight, ColorMode::Rgb);
    document.layers = layers;
    document
}

fn pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let value = image.get(x, y);
    value.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn close(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2)
}

#[test]
fn separable_blend_modes_follow_their_formulas() {
    let cases: [(BlendMode, [u8; 3]); 8] = [
        (BlendMode::Normal, [64, 128, 191]),
        (BlendMode::Multiply, [32, 128, 95]),
        (BlendMode::Screen, [160, 255, 223]),
        (BlendMode::Difference, [64, 127, 63]),
        (BlendMode::Darken, [64, 128, 128]),
        (BlendMode::Lighten, [128, 255, 191]),
        (BlendMode::LinearDodge, [192, 255, 255]),
        (BlendMode::Subtract, [64, 127, 0]),
    ];
    for (mode, expected) in cases {
        let base = solid("base", Rect::new(0, 0, 2, 2), [128, 255, 128, 255]);
        let mut top = solid("top", Rect::new(0, 0, 2, 2), [64, 128, 191, 255]);
        top.blend_mode = mode;
        let image = render_rgba(&document(2, 2, vec![base.into(), top.into()])).unwrap();
        let got = pixel(&image, 1, 1);
        assert!(
            close(got, [expected[0], expected[1], expected[2], 255]),
            "{mode:?}: {got:?}"
        );
    }
}

#[test]
fn isolated_groups_differ_from_pass_through() {
    let base = solid("base", Rect::new(0, 0, 1, 1), [128, 128, 128, 255]);
    let mut child = solid("child", Rect::new(0, 0, 1, 1), [128, 128, 128, 255]);
    child.blend_mode = BlendMode::Multiply;
    let mut pass = Group::new("pass");
    pass.children = vec![child.clone().into()];
    let image = render_rgba(&document(1, 1, vec![base.clone().into(), pass.into()])).unwrap();
    assert!(close(pixel(&image, 0, 0), [64, 64, 64, 255]));
    let mut isolated = Group::new("isolated");
    isolated.set_blend_mode(BlendMode::Normal);
    isolated.children = vec![child.into()];
    let image = render_rgba(&document(1, 1, vec![base.into(), isolated.into()])).unwrap();
    assert!(close(pixel(&image, 0, 0), [128, 128, 128, 255]));
}

#[test]
fn shallow_knockout_in_pass_through_reveals_backdrop() {
    let base = solid("base", Rect::new(0, 0, 1, 1), [255, 0, 0, 255]);
    let inside = solid("inside", Rect::new(0, 0, 1, 1), [0, 255, 0, 255]);
    let mut punch = solid("punch", Rect::new(0, 0, 1, 1), [0, 0, 255, 255]);
    punch.set_fill_opacity(0.0);
    punch.info.push(Info::Knockout(Knockout::Shallow));
    let mut pass = Group::new("pass");
    pass.children = vec![inside.into(), punch.into()];
    let image = render_rgba(&document(1, 1, vec![base.into(), pass.into()])).unwrap();
    assert!(close(pixel(&image, 0, 0), [255, 0, 0, 255]));
}

#[test]
fn masks_and_clipping_limit_coverage() {
    let mut masked = solid("masked", Rect::new(0, 0, 4, 1), [255, 0, 0, 255]);
    let mask = Plane::from_bytes(2, 1, Depth::Eight, &[255, 0]).unwrap();
    masked
        .set_mask(
            LayerMask::new(Rect::new(0, 0, 2, 1), 0),
            &mask,
            Compression::Rle,
        )
        .unwrap();
    let image = render_rgba(&document(4, 1, vec![masked.into()])).unwrap();
    assert_eq!(pixel(&image, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 1, 0)[3], 0);
    assert_eq!(pixel(&image, 3, 0)[3], 0);

    for (outside, alpha) in [(0, 0), (255, 255)] {
        let mut beside = solid("beside", Rect::new(0, 0, 2, 1), [255, 0, 0, 255]);
        let mask = Plane::from_bytes(1, 1, Depth::Eight, &[0]).unwrap();
        beside
            .set_mask(
                LayerMask::new(Rect::new(3, 0, 4, 1), outside),
                &mask,
                Compression::Rle,
            )
            .unwrap();
        let image = render_rgba(&document(4, 1, vec![beside.into()])).unwrap();
        assert_eq!(pixel(&image, 1, 0)[3], alpha);
    }

    let base = solid("base", Rect::new(0, 0, 2, 1), [0, 0, 255, 255]);
    let mut clipped = solid("clipped", Rect::new(0, 0, 4, 1), [0, 255, 0, 255]);
    clipped.clipping = true;
    let image = render_rgba(&document(4, 1, vec![base.into(), clipped.into()])).unwrap();
    assert_eq!(pixel(&image, 1, 0), [0, 255, 0, 255]);
    assert_eq!(pixel(&image, 2, 0)[3], 0);
}

#[test]
fn adjustment_layers_modify_backdrop() {
    let base = solid("base", Rect::new(0, 0, 1, 1), [10, 100, 200, 255]);
    let mut invert = Layer::new("invert");
    invert.set_info(Info::Adjustment(Adjustment::Invert));
    let image = render_rgba(&document(1, 1, vec![base.clone().into(), invert.into()])).unwrap();
    assert!(close(pixel(&image, 0, 0), [245, 155, 55, 255]));

    let mut threshold = Layer::new("threshold");
    threshold.set_info(Info::Adjustment(Adjustment::Threshold { level: 128 }));
    let image = render_rgba(&document(1, 1, vec![base.into(), threshold.into()])).unwrap();
    assert_eq!(pixel(&image, 0, 0), [0, 0, 0, 255]);
}

#[test]
fn layer_effects_render_around_and_over() {
    let mut layer = solid("box", Rect::new(4, 4, 8, 8), [255, 255, 255, 255]);
    let mut effects = Effects::new();
    effects
        .color_overlays
        .push(Overlay::color(Color::rgb8(255, 0, 0)));
    let mut stroke = Stroke::new(2.0, Color::rgb8(0, 0, 255));
    stroke.position = rikka_psd::StrokePosition::Outside;
    effects.strokes.push(stroke);
    let mut shadow = Shadow::new(Color::BLACK);
    shadow.size = 0.0;
    shadow.distance = 3.0;
    shadow.angle = 90.0;
    shadow.use_global_light = false;
    shadow.common.opacity = 1.0;
    shadow.common.blend_mode = BlendMode::Normal;
    effects.drop_shadows.push(shadow);
    layer.set_effects(Some(&effects));
    let image = render_rgba(&document(12, 16, vec![layer.into()])).unwrap();
    assert!(close(pixel(&image, 5, 5), [255, 0, 0, 255]));
    assert!(close(pixel(&image, 3, 5), [0, 0, 255, 255]));
    assert_eq!(pixel(&image, 0, 0)[3], 0);
    assert!(
        close(pixel(&image, 5, 10), [0, 0, 0, 255]),
        "{:?}",
        pixel(&image, 5, 10)
    );
}

fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> PathData {
    PathData::new(vec![Subpath::new(
        true,
        PathOperation::Combine,
        vec![
            Knot::corner(Point { x: left, y: top }),
            Knot::corner(Point { x: right, y: top }),
            Knot::corner(Point {
                x: right,
                y: bottom,
            }),
            Knot::corner(Point { x: left, y: bottom }),
        ],
    )])
}

#[test]
fn fill_layers_are_synthesized_from_their_settings() {
    let mut fill = Layer::new("Color Fill");
    fill.set_fill(&FillContent::Solid(Color::rgb8(0, 128, 0)));
    fill.set_info(Info::VectorMask(VectorMask::new(rectangle(
        0.25, 0.25, 0.75, 0.75,
    ))));
    let image = render_rgba(&document(8, 8, vec![fill.into()])).unwrap();
    assert_eq!(pixel(&image, 4, 4), [0, 128, 0, 255]);
    assert_eq!(pixel(&image, 1, 1)[3], 0);
    let covered: f32 = image.pixels.iter().map(|pixel| pixel[3]).sum();
    assert!((covered - 16.0).abs() < 0.01, "{covered}");
}

#[test]
fn vector_masks_rasterize_with_partial_coverage() {
    let triangle = PathData::new(vec![Subpath::new(
        true,
        PathOperation::Combine,
        vec![
            Knot::corner(Point { x: 0.0, y: 0.0 }),
            Knot::corner(Point { x: 1.0, y: 0.0 }),
            Knot::corner(Point { x: 0.0, y: 1.0 }),
        ],
    )]);
    let mut fill = Layer::new("Color Fill");
    fill.set_fill(&FillContent::Solid(Color::rgb8(0, 128, 0)));
    fill.set_info(Info::VectorMask(VectorMask::new(triangle)));
    let image = render_rgba(&document(10, 10, vec![fill.into()])).unwrap();
    let covered: f32 = image.pixels.iter().map(|pixel| pixel[3]).sum();
    assert!((covered - 50.0).abs() < 0.05, "{covered}");
    assert!((image.get(5, 4)[3] - 0.5).abs() < 0.01);
}

#[test]
fn cmyk_documents_composite_in_ink_space() {
    let mut document = Document::new(1, 1, Depth::Eight, ColorMode::Cmyk);
    let mut base = Layer::new("base");
    let mut multiply = Layer::new("multiply");
    for (layer, values) in [
        (&mut base, [0.5f32, 1.0, 1.0, 1.0]),
        (&mut multiply, [1.0, 0.5, 1.0, 1.0]),
    ] {
        let planes: Vec<(ChannelId, Plane)> = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                (
                    ChannelId::Color(index as u16),
                    Plane::from_normalized(1, 1, Depth::Eight, &[*value]).unwrap(),
                )
            })
            .collect();
        layer
            .set_pixels(Rect::new(0, 0, 1, 1), &planes, Compression::Raw)
            .unwrap();
    }
    multiply.blend_mode = BlendMode::Multiply;
    document.layers = vec![base.into(), multiply.into()];
    let renderer = Renderer::new(&document);
    let raster = renderer.render().unwrap();
    let pixel = raster.pixel(0, 0).unwrap();
    assert!(
        (pixel[0] - 0.5).abs() < 0.01
            && (pixel[1] - 0.5).abs() < 0.01
            && (pixel[3] - 1.0).abs() < 0.01
    );
}
