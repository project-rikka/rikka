use rikka_gpu::Pixels;
use rikka_renderer::{Error, Node, Scene, TextureId};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-textures")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn fill(width: u32, height: u32, texel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| texel(x, y))
        .collect()
}

fn drawn(renderer: &mut TestRenderer, texture: TextureId, side: u32) -> Vec<u8> {
    let mut target = renderer.create_target(side, side).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, side as f32, side as f32),
        texture,
    ));
    renderer.draw(&mut scene, &mut target).unwrap();
    renderer.read_pixels(&mut target).unwrap()
}

#[test]
fn rejects_empty_texture() {
    let mut renderer = renderer();
    let textures = renderer.textures();
    assert!(matches!(
        textures.create(Pixels::premultiplied(0, 4, Vec::new())),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(textures.blank(4, 0), Err(Error::Invalid(_))));
}

#[test]
fn rejects_texture_beyond_device_limit() {
    let mut renderer = renderer();
    assert!(matches!(
        renderer.textures().blank(1 << 20, 1),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn overlapping_writes_apply_in_order() {
    let mut renderer = renderer();
    let texture = renderer
        .textures()
        .create(Pixels::premultiplied(
            16,
            16,
            fill(16, 16, |_, _| [0, 0, 255, 255]),
        ))
        .unwrap();
    let textures = renderer.textures();
    textures
        .update_region(
            texture,
            [0, 0],
            Pixels::premultiplied(8, 8, fill(8, 8, |_, _| [255, 255, 255, 255])),
        )
        .unwrap();
    textures
        .update_region(
            texture,
            [4, 4],
            Pixels::premultiplied(8, 8, fill(8, 8, |_, _| [0, 0, 0, 255])),
        )
        .unwrap();
    drop(textures);
    let pixels = drawn(&mut renderer, texture, 16);
    let texel = |x: u32, y: u32| {
        let at = ((y * 16 + x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    assert_eq!(
        texel(2, 2),
        [255, 255, 255],
        "only the first write reaches here"
    );
    assert_eq!(
        texel(6, 6),
        [0, 0, 0],
        "the second write lands over the first"
    );
    assert_eq!(
        texel(14, 14),
        [0, 0, 255],
        "untouched texels keep their blue"
    );
    renderer.textures().destroy(texture);
}

#[test]
fn minified_image_samples_mipmaps() {
    let mut renderer = renderer();
    let seams = |at: u32| matches!(at % 16, 7 | 8);
    let texture = renderer
        .textures()
        .create(Pixels::premultiplied(
            64,
            64,
            fill(64, 64, |x, y| match seams(x) || seams(y) {
                true => [0, 0, 0, 255],
                false => [255, 255, 255, 255],
            }),
        ))
        .unwrap();
    let full = drawn(&mut renderer, texture, 64);
    assert_eq!(
        &full[..4],
        &[255, 255, 255, 255],
        "drawn whole, it is unchanged"
    );
    let small = drawn(&mut renderer, texture, 4);
    for pixel in small.as_chunks::<4>().0 {
        assert!(
            (150..=230).contains(&pixel[0]),
            "a quarter-size pixel blends its 16x16 texels, not the seam it lands on: {pixel:?}"
        );
    }
    renderer.textures().destroy(texture);
}
