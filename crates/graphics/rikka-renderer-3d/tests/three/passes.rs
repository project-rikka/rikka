use rikka_gpu::{Filtering, Pixels};
use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Draw, Frame, Indices, Light, MaterialDesc, MaterialId, MaterialState,
    MeshData, MeshId, Outline, ShadowQuality, TextureBinding, ToneMapping, Viewport,
    ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 128;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-three-passes-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> (u8, u8, u8, u8) {
    let i = (y * SIZE as usize + x) * 4;
    (pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3])
}

fn mesh(
    renderer: &mut Renderer,
    positions: &[Vec3],
    normals: &[Vec3],
    indices: Indices<'_>,
) -> MeshId {
    let tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); positions.len()];
    let uvs: Vec<Vec2> = positions
        .iter()
        .map(|position| position.truncate() * 0.5 + 0.5)
        .collect();
    renderer
        .three()
        .create_mesh(&MeshData {
            positions,
            normals,
            tangents: &tangents,
            uvs: &uvs,
            indices: Some(indices),
            ..MeshData::default()
        })
        .expect("a mesh")
}

fn quad(renderer: &mut Renderer, z: f32, half: f32) -> MeshId {
    let positions = [
        Vec3::new(-half, -half, z),
        Vec3::new(half, -half, z),
        Vec3::new(half, half, z),
        Vec3::new(-half, half, z),
    ];
    mesh(
        renderer,
        &positions,
        &[Vec3::Z; 4],
        Indices::U32(&[0, 1, 2, 0, 2, 3]),
    )
}

fn short_quad(renderer: &mut Renderer, z: f32, half: f32) -> MeshId {
    let positions = [
        Vec3::new(-half, -half, z),
        Vec3::new(half, -half, z),
        Vec3::new(half, half, z),
        Vec3::new(-half, half, z),
    ];
    mesh(
        renderer,
        &positions,
        &[Vec3::Z; 4],
        Indices::U16(&[0, 1, 2, 0, 2, 3]),
    )
}

fn pbr(color: [f32; 4], state: MaterialState) -> MaterialDesc {
    rikka_test_renderer_3d::standard(Vec4::from_array(color), state)
}

fn material(renderer: &mut Renderer, desc: MaterialDesc) -> MaterialId {
    renderer.three().create_material(desc).expect("a material")
}

fn camera() -> Camera {
    Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    )
}

fn frame(ambient: f32) -> Frame {
    let mut frame = Frame::new(camera());
    frame
        .background(Background::Transparent)
        .ambient(Vec3::splat(ambient))
        .tone_mapping(ToneMapping::Clamp)
        .outline(Outline {
            enabled: false,
            ..Default::default()
        });
    frame
}

fn viewport(renderer: &mut Renderer, samples: u32) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(samples))
        .expect("a viewport")
}

fn read(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Vec<u8> {
    renderer
        .three()
        .render(viewport, frame)
        .expect("a 3D frame draws");
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

fn shadow_under_masked_caster(state: MaterialState) -> u8 {
    const UNDER_THE_CASTER: usize = 96;

    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let wall = quad(&mut renderer, -2.0, 4.0);
    let caster = quad(&mut renderer, 0.0, 0.6);
    let clear = renderer
        .textures()
        .create(Pixels::rgba8(4, 4, vec![0u8; 64]))
        .expect("a texture");
    let white = pbr([1.0; 4], MaterialState::opaque());
    let white = material(&mut renderer, white);
    let cut = pbr([1.0; 4], state)
        .with(
            "base_color_texture",
            TextureBinding::new(clear, Filtering::NearestClamp),
        )
        .expect("a cut-out texture");
    let cut = material(&mut renderer, cut);

    let mut frame = frame(0.0);
    frame.light(
        Light::directional(Vec3::new(0.0, -0.6, -1.0))
            .intensity(4.0)
            .shadow(ShadowQuality::Medium),
    );
    frame.draw(Draw::new(wall, white));
    frame.draw(Draw::new(caster, cut));
    let pixels = read(&mut renderer, &mut view, &frame);
    pixel(&pixels, 64, UNDER_THE_CASTER).0
}

#[test]
fn masked_caster_passes_light_where_transparent() {
    let lit = shadow_under_masked_caster(MaterialState::masked());
    assert!(lit > 200, "the band under a see-through caster: {lit}");
}

fn revealed(samples: u32) {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, samples);
    let wall = quad(&mut renderer, 1.0, 3.0);
    let hidden = quad(&mut renderer, -1.0, 0.3);
    let grey = pbr([0.2, 0.2, 0.2, 1.0], MaterialState::opaque());
    let grey = material(&mut renderer, grey);
    let red = pbr([1.0, 0.0, 0.0, 1.0], MaterialState::opaque());
    let red = material(&mut renderer, red);

    let mut walled = frame(1.0);
    walled.draw(Draw::new(wall, grey));
    walled.draw(Draw::new(hidden, red));
    read(&mut renderer, &mut view, &walled);
    let covered = read(&mut renderer, &mut view, &walled);
    let (r, g, b, _) = pixel(&covered, 64, 64);
    assert!(
        r < 160 && r == g && g == b,
        "the wall is in front: {r},{g},{b}"
    );

    let mut open = frame(1.0);
    open.draw(Draw::new(hidden, red));
    let pixels = read(&mut renderer, &mut view, &open);
    let (r, g, b, _) = pixel(&pixels, 64, 64);
    assert!(
        r > 200 && g < 60 && b < 60,
        "what the last frame hid shows once nothing hides it: {r},{g},{b}"
    );
}

#[test]
fn occlusion_culled_mesh_reappears_when_unoccluded() {
    revealed(1);
}

#[test]
fn partially_occluded_mesh_stays_drawn() {
    let mut renderer = renderer();
    let mut view = renderer
        .three()
        .create_viewport(ViewportConfig::new(120, SIZE).samples(4))
        .expect("a viewport");
    let wall = quad(&mut renderer, 1.0, 1.5);
    let peeking = quad(&mut renderer, -1.0, 0.5);
    let grey = pbr([0.2, 0.2, 0.2, 1.0], MaterialState::opaque());
    let grey = material(&mut renderer, grey);
    let red = pbr([1.0, 0.0, 0.0, 1.0], MaterialState::opaque());
    let red = material(&mut renderer, red);

    let mut frame = frame(1.0);
    frame.draw(Draw::new(wall, grey).transform(Mat4::from_translation(Vec3::new(-1.5, 0.0, 0.0))));
    frame.draw(Draw::new(peeking, red).transform(Mat4::from_translation(Vec3::new(0.3, 0.0, 0.0))));
    for _ in 0..3 {
        let pixels = read(&mut renderer, &mut view, &frame);
        let (r, g, b, _) = pixel(&pixels, 80, 64);
        assert!(
            r > 200 && g < 60 && b < 60,
            "the part the wall leaves: {r},{g},{b}"
        );
        let (r, g, b, _) = pixel(&pixels, 40, 64);
        assert!(r < 160 && r == g && g == b, "the wall: {r},{g},{b}");
    }
}

#[test]
fn u16_and_u32_index_meshes_draw_alike() {
    const UNDER_THE_CASTER: usize = 96;

    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 4);
    let wall = quad(&mut renderer, -2.0, 4.0);
    let caster = short_quad(&mut renderer, 0.0, 0.6);
    let pane = short_quad(&mut renderer, 1.0, 0.1);
    let white = pbr([1.0; 4], MaterialState::opaque());
    let white = material(&mut renderer, white);
    let glass = pbr([1.0, 0.0, 0.0, 0.5], MaterialState::transparent());
    let glass = material(&mut renderer, glass);

    let mut frame = frame(0.0);
    frame.light(
        Light::directional(Vec3::new(0.0, -0.6, -1.0))
            .intensity(4.0)
            .shadow(ShadowQuality::Medium),
    );
    frame.draw(Draw::new(wall, white));
    frame.draw(Draw::new(caster, white));
    frame.draw(Draw::new(pane, glass).transform(Mat4::from_translation(Vec3::new(0.0, 0.8, 0.0))));
    let pixels = read(&mut renderer, &mut view, &frame);

    let (r, g, b, _) = pixel(&pixels, 64, 64);
    assert!(
        r > 200 && g > 200 && b > 200,
        "the caster itself: {r},{g},{b}"
    );
    let (r, _, _, _) = pixel(&pixels, 64, UNDER_THE_CASTER);
    assert_eq!(r, 0, "the shadow of a sixteen-bit caster");
    let column: Vec<(u8, u8)> = (0..64)
        .map(|y| {
            let (r, g, _, _) = pixel(&pixels, 64, y);
            (r, g)
        })
        .collect();
    let pane_row = column
        .iter()
        .position(|&(r, g)| r > g.saturating_add(40))
        .unwrap_or_else(|| {
            panic!("the sixteen-bit glass pane tints what is behind it: {column:?}")
        });
    assert!(
        pane_row < 40,
        "the pane sits above the caster: row {pane_row}"
    );
}
