use rikka_renderer::{BackdropPass, BackdropShader, Node, PassList, PostEffect, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect, Size};

const SIDE: u32 = 128;

struct Softened(BackdropShader);

impl PostEffect for Softened {
    fn name(&self) -> &str {
        "softened"
    }

    fn passes(&self, _region: Size, list: &mut PassList) {
        list.push(
            BackdropPass::new(self.0.clone())
                .params(&[1.0])
                .scale(0.5)
                .taps(2.0),
        );
    }
}

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-custom-effects")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn softened_scene(shader: BackdropShader) -> Scene {
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(
        Node::rect(Rect::new(0.0, 0.0, SIDE as f32 / 2.0, SIDE as f32))
            .fill(Color::rgb(0.0, 0.0, 1.0)),
    );
    scene.set_effects(vec![Box::new(Softened(shader))]);
    scene
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> (u8, u8, u8, u8) {
    let at = ((y * SIDE + x) * 4) as usize;
    (pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3])
}

#[test]
fn custom_effect_filters_scene() {
    let before = BackdropShader::new(shaders::BLUR_DOWN_FRAG);
    let mut renderer = renderer();
    let after = BackdropShader::new(shaders::BLUR_DOWN_FRAG);

    for shader in [before, after] {
        let mut target = renderer.create_target(SIDE, SIDE).unwrap();
        let mut scene = softened_scene(shader);
        renderer.draw(&mut scene, &mut target).unwrap();
        let pixels = renderer.read_pixels(&mut target).unwrap();

        assert_eq!(pixel(&pixels, 16, 64), (0, 0, 255, 255), "inside the rect");
        assert_eq!(pixel(&pixels, 112, 64), (0, 0, 0, 255), "far from the rect");
        let (_, _, edge, _) = pixel(&pixels, 64, 64);
        assert!(
            edge > 16 && edge < 240,
            "the edge is softened, not cut: {edge}"
        );
    }
}
