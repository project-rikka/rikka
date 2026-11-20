use rikka_colorimetry::{
    BlendSpace, Chromaticity, Description, HdrSpace, Intent, Mapping, PQ_PEAK, Primaries,
    TransferFunction,
};
use rikka_gpu::Pixels;
use rikka_renderer::{AlphaMode, Blur, Error, ImageColor, ImageSource, Node, Scene, TargetFormat};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIDE: u32 = 8;
const HDR: HdrSpace = HdrSpace {
    reference: 203.0,
    peak: 1000.0,
};

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-color")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn whole() -> Rect {
    Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32)
}

fn centre(pixels: &[u8]) -> [u8; 3] {
    let at = ((SIDE / 2 * SIDE + SIDE / 2) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

fn bytes(values: [f64; 3]) -> [u8; 3] {
    values.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn assert_near(actual: [u8; 3], expected: [u8; 3], what: &str) {
    let off = actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| actual.abs_diff(expected))
        .max()
        .unwrap_or(0);
    assert!(off <= 2, "{what}: drew {actual:?}, expected {expected:?}");
}

fn solid_texture(renderer: &mut TestRenderer, texel: [u8; 4]) -> rikka_renderer::TextureId {
    let pixels: Vec<u8> = (0..SIDE * SIDE).flat_map(|_| texel).collect();
    renderer
        .textures()
        .create(Pixels::premultiplied(SIDE, SIDE, pixels))
        .unwrap()
}

fn drawn(renderer: &mut TestRenderer, space: BlendSpace, node: Node) -> [u8; 3] {
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    target.set_blend_space(space);
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(node);
    renderer.draw(&mut scene, &mut target).unwrap();
    centre(&renderer.read_pixels(&mut target).unwrap())
}

#[test]
fn srgb_content_is_pq_encoded_on_hdr_targets() {
    let mut renderer = renderer();
    let mapping = Mapping::new(
        &Description::SRGB,
        Intent::Perceptual,
        &BlendSpace::Hdr(HDR),
    )
    .unwrap();

    let red = drawn(
        &mut renderer,
        BlendSpace::Hdr(HDR),
        Node::rect(whole()).fill(Color::rgb(1.0, 0.0, 0.0)),
    );
    assert_near(
        red,
        bytes(HDR.encode(mapping.apply([1.0, 0.0, 0.0]))),
        "sRGB red fill",
    );

    let grey = solid_texture(&mut renderer, [128, 128, 128, 255]);
    let image = drawn(
        &mut renderer,
        BlendSpace::Hdr(HDR),
        Node::image(whole(), grey),
    );
    let encoded = 128.0 / 255.0;
    assert_near(
        image,
        bytes(HDR.encode(mapping.apply([encoded; 3]))),
        "sRGB grey image",
    );
    assert_near(
        image,
        bytes([TransferFunction::Pq.encode(encoded.powf(2.2) * 203.0 / PQ_PEAK); 3]),
        "grey keeps its share of reference white",
    );
}

#[test]
fn ten_bit_targets_carry_pq_at_ten_bit_precision() {
    let mut renderer = renderer();
    let mut target = renderer
        .create_target_as(SIDE, SIDE, TargetFormat::Rgb10a2)
        .unwrap();
    target.set_blend_space(BlendSpace::Hdr(HDR));
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(Node::rect(whole()).fill(Color::rgb(1.0, 0.5, 0.0)));
    renderer.draw(&mut scene, &mut target).unwrap();
    let pixels = renderer.read_pixels(&mut target).unwrap();
    let at = ((SIDE / 2 * SIDE + SIDE / 2) * 4) as usize;
    let word = u32::from_le_bytes([pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]);
    let channels = [word & 0x3ff, (word >> 10) & 0x3ff, (word >> 20) & 0x3ff];
    let expected = HDR
        .encode(
            Mapping::new(
                &Description::SRGB,
                Intent::Perceptual,
                &BlendSpace::Hdr(HDR),
            )
            .unwrap()
            .apply([1.0, 0.5, 0.0]),
        )
        .map(|channel| (channel * 1023.0).round() as u32);
    for (channel, expected) in channels.iter().zip(expected) {
        assert!(
            channel.abs_diff(expected) <= 2,
            "drew {channels:?}, expected {expected:?}"
        );
    }
    assert_eq!(word >> 30, 0b11, "opaque");
}

#[test]
fn glass_over_hdr_content_blurs_in_the_blend_space() {
    let mut renderer = renderer();
    let ground = Color::rgb(0.2, 0.6, 0.9);
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    target.set_blend_space(BlendSpace::Hdr(HDR));
    let mut scene = Scene::new();
    scene.insert(Node::rect(whole()).fill(ground));
    scene.insert(
        Node::rect(whole())
            .fill(Color::TRANSPARENT)
            .backdrop(Blur::new(2.0))
            .z(1),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let blurred = centre(&renderer.read_pixels(&mut target).unwrap());
    let plain = drawn(
        &mut renderer,
        BlendSpace::Hdr(HDR),
        Node::rect(whole()).fill(ground),
    );
    assert_near(blurred, plain, "a uniform ground blurs to itself");
}

#[test]
fn pq_images_are_tone_mapped_on_sdr_targets() {
    let mut renderer = renderer();
    let color = ImageColor {
        description: Description {
            max_cll: Some(1000.0),
            ..Description::BT2100_PQ
        },
        intent: Intent::Perceptual,
        alpha: AlphaMode::Premultiplied,
    };
    let id = renderer.colors().acquire(color).unwrap();
    let mapping = Mapping::new(&color.description, color.intent, &BlendSpace::Sdr).unwrap();
    for (nits, what) in [
        (1000.0, "peak"),
        (203.0, "reference white"),
        (20.0, "shadow"),
    ] {
        let encoded = TransferFunction::Pq.encode(nits / PQ_PEAK);
        let byte = (encoded * 255.0).round() as u8;
        let texture = solid_texture(&mut renderer, [byte, byte, byte, 255]);
        let drawn = drawn(
            &mut renderer,
            BlendSpace::Sdr,
            Node::image(whole(), texture).source(ImageSource::whole().color(id)),
        );
        let sampled = f64::from(byte) / 255.0;
        assert_near(drawn, bytes(mapping.apply([sampled; 3])), what);
    }
    renderer.colors().release(id);
}

#[test]
fn straight_alpha_images_are_premultiplied_when_drawn() {
    let mut renderer = renderer();
    let id = renderer
        .colors()
        .acquire(ImageColor {
            alpha: AlphaMode::Straight,
            ..ImageColor::default()
        })
        .unwrap();
    let texture = solid_texture(&mut renderer, [255, 255, 255, 128]);
    let drawn = drawn(
        &mut renderer,
        BlendSpace::Sdr,
        Node::image(whole(), texture).source(ImageSource::whole().color(id)),
    );
    assert_near(drawn, [128, 128, 128], "half-covered white over black");
}

#[test]
fn degenerate_image_colors_are_rejected() {
    let mut renderer = renderer();
    let collinear = Primaries::new(
        Chromaticity::new(0.2, 0.2),
        Chromaticity::new(0.3, 0.3),
        Chromaticity::new(0.4, 0.4),
        Chromaticity::D65,
    );
    let refused = renderer.colors().acquire(ImageColor {
        description: Description {
            primaries: collinear,
            ..Description::SRGB
        },
        ..ImageColor::default()
    });
    assert!(matches!(refused, Err(Error::Invalid(_))));
}
