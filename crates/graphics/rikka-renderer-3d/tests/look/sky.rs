use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Background, Camera, Frame, Look, Outline, Viewport, ViewportConfig, Warming,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 64;

const HIGH: (u32, u32) = (SIZE / 2, 4);

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-sky-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn clear_look(hour: f32, adaptation: f32) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    rikka_test_renderer_3d::sky_of(&mut look).hour = hour;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = adaptation;
    look
}

fn sky_frame(look: Look, towards: Vec3) -> Frame {
    let eye = Vec3::new(0.0, 1.5, 0.0);
    let mut frame = Frame::new(Camera::look_at(eye, eye + towards, Vec3::Y, 1.6, 1.0, 0.05));
    frame
        .background(Background::Sky)
        .ambient(Vec3::ZERO)
        .look(look)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

fn sky(renderer: &mut Renderer, look: Look, towards: Vec3) -> Vec<u8> {
    let frame = sky_frame(look, towards);
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE))
        .expect("a viewport");
    for _ in 0..3 {
        renderer
            .three()
            .render(&mut viewport, &frame)
            .expect("a 3D frame draws");
    }
    pixels(renderer, &viewport)
}

fn pixels(renderer: &mut Renderer, viewport: &Viewport) -> Vec<u8> {
    let mut target = renderer
        .create_target(SIZE, SIZE)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    renderer.read_pixels(&mut target).expect("pixels come back")
}

fn at(pixels: &[u8], (x, y): (u32, u32)) -> [f32; 3] {
    let i = ((y * SIZE + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2]].map(f32::from)
}

#[test]
fn noon_sky_is_blue_at_zenith() {
    let mut test = renderer();
    let pixels = sky(&mut test, clear_look(12.0, 0.0), Vec3::new(0.0, 0.6, -1.0));
    let [r, g, b] = at(&pixels, HIGH);
    assert!(b > r + 20.0 && b > g, "noon zenith {r} {g} {b}");
}

#[test]
fn plain_warming_converges_to_the_looked_frame() {
    let mut test = renderer();
    let towards = Vec3::new(0.0, 0.6, -1.0);
    let frame = sky_frame(clear_look(12.0, 0.0), towards);
    let mut viewport = test
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).warming(Warming::DrawPlain))
        .expect("a viewport");
    loop {
        test.three()
            .render(&mut viewport, &frame)
            .expect("a 3D frame draws");
        if !viewport.is_converging() {
            break;
        }
    }
    for _ in 0..2 {
        test.three()
            .render(&mut viewport, &frame)
            .expect("a 3D frame draws");
    }
    let warmed = pixels(&mut test, &viewport);
    let waited = sky(&mut test, clear_look(12.0, 0.0), towards);
    let [r, g, b] = at(&warmed, HIGH);
    assert_eq!([r, g, b], at(&waited, HIGH), "warmed zenith {r} {g} {b}");
}
