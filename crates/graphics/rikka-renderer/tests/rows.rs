use rikka_gpu::Rows;
use rikka_renderer::{Node, Scene, TexelRect, TextureId};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIDE: u32 = 8;

fn renderer() -> TestRenderer {
    rikka_test_renderer::streaming_renderer("rikka-renderer-rows")
        .expect("a Vulkan 1.3 device is required for these tests")
}

const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*code)
}

fn drawn(renderer: &mut TestRenderer, texture: TextureId) -> Vec<[u8; 4]> {
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 1.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32),
        texture,
    ));
    renderer.draw(&mut scene, &mut target).unwrap();
    let pixels = renderer.read_pixels(&mut target).unwrap();
    pixels.as_chunks::<4>().0.to_vec()
}

fn picture(texel: &[u8], stride: usize) -> Vec<u8> {
    let mut rows = vec![0u8; stride * SIDE as usize];
    for row in rows.chunks_exact_mut(stride) {
        for at in (0..SIDE as usize * texel.len()).step_by(texel.len()) {
            row[at..at + texel.len()].copy_from_slice(texel);
        }
    }
    rows
}

#[test]
fn drm_layout_rows_draw_their_colors() {
    let red_half = [0xf0, 0x80, 0x10, 0xff];
    let cases: [(&str, u32, Vec<u8>, [u8; 4]); 8] = [
        (
            "argb8888",
            fourcc(b"AR24"),
            vec![0x10, 0x80, 0xf0, 0xff],
            red_half,
        ),
        (
            "xrgb8888 ignores alpha",
            fourcc(b"XR24"),
            vec![0x10, 0x80, 0xf0, 0x00],
            red_half,
        ),
        (
            "abgr8888",
            fourcc(b"AB24"),
            vec![0xf0, 0x80, 0x10, 0xff],
            red_half,
        ),
        (
            "xbgr8888 ignores alpha",
            fourcc(b"XB24"),
            vec![0xf0, 0x80, 0x10, 0x00],
            red_half,
        ),
        (
            "rgb565",
            fourcc(b"RG16"),
            0xf800u16.to_le_bytes().to_vec(),
            [0xff, 0, 0, 0xff],
        ),
        (
            "abgr2101010",
            fourcc(b"AB30"),
            (3u32 << 30 | 1023 << 10).to_le_bytes().to_vec(),
            [0, 0xff, 0, 0xff],
        ),
        (
            "xrgb2101010 ignores alpha",
            fourcc(b"XR30"),
            (1023u32 << 20).to_le_bytes().to_vec(),
            [0xff, 0, 0, 0xff],
        ),
        (
            "abgr16161616f",
            fourcc(b"AB4H"),
            [0x3c00u16, 0, 0x3c00, 0x3c00]
                .iter()
                .flat_map(|half| half.to_le_bytes())
                .collect(),
            [0xff, 0, 0xff, 0xff],
        ),
    ];
    let mut renderer = renderer();
    for (name, code, texel, expected) in cases {
        if !AsRef::<rikka_renderer::Uploads>::as_ref(&renderer).supports_drm_format(code) {
            continue;
        }
        let stride = SIDE as usize * texel.len() + 12;
        let texture = renderer.textures().create_drm(code, SIDE, SIDE).unwrap();
        let rows = picture(&texel, stride);
        let whole = [TexelRect::new([0, 0], [SIDE, SIDE])];
        renderer
            .textures()
            .stage(texture, Rows::new(&rows, stride), &whole)
            .unwrap()
            .send()
            .unwrap();
        let seen = drawn(&mut renderer, texture);
        assert!(
            seen.iter().all(|pixel| *pixel == expected),
            "{name}: {:?} instead of {expected:?}",
            seen[0]
        );
        renderer.textures().destroy(texture);
    }
}

#[test]
fn staged_rects_write_only_their_texels() {
    let mut renderer = renderer();
    let code = fourcc(b"XR24");
    let stride = SIDE as usize * 4;
    let texture = renderer.textures().create_drm(code, SIDE, SIDE).unwrap();
    let whole = [TexelRect::new([0, 0], [SIDE, SIDE])];
    let blue = picture(&[0xff, 0, 0, 0], stride);
    renderer
        .textures()
        .stage(texture, Rows::new(&blue, stride), &whole)
        .unwrap()
        .send()
        .unwrap();
    assert!(
        drawn(&mut renderer, texture)
            .iter()
            .all(|pixel| *pixel == [0, 0, 0xff, 0xff])
    );

    let green = picture(&[0, 0xff, 0, 0], stride);
    let sparse = [
        TexelRect::new([0, 0], [2, 2]),
        TexelRect::new([5, 6], [3, 2]),
    ];
    renderer
        .textures()
        .stage(texture, Rows::new(&green, stride), &sparse)
        .unwrap()
        .send()
        .unwrap();
    let seen = drawn(&mut renderer, texture);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let inside = sparse.iter().any(|rect| {
                (rect.offset[0]..rect.offset[0] + rect.extent[0]).contains(&x)
                    && (rect.offset[1]..rect.offset[1] + rect.extent[1]).contains(&y)
            });
            let expected = match inside {
                true => [0, 0xff, 0, 0xff],
                false => [0, 0, 0xff, 0xff],
            };
            assert_eq!(seen[(y * SIDE + x) as usize], expected, "texel {x},{y}");
        }
    }

    let short = vec![0u8; stride];
    assert!(
        renderer
            .textures()
            .stage(texture, Rows::new(&short, stride), &whole)
            .is_err(),
        "rows that end early are refused"
    );
    renderer.textures().destroy(texture);
}
