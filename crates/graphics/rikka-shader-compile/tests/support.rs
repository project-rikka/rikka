use rikka_gpu::{Filtering, Pixels};
use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Draw, Frame, Indices, Light, MaterialDesc, MaterialId, MaterialTemplate,
    MeshData, MeshId, Outline, ShaderCompiler, ShadowQuality, TextureBinding, ToneMapping,
    ViewportConfig,
};
use rikka_shader_compile::Material;
use rikka_shader_graph::{Graph, Library};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

pub(crate) const SCREEN_HALF: f32 = 2.185_204;

pub(crate) fn quad(renderer: &mut Renderer, half: f32) -> MeshId {
    let positions = [
        Vec3::new(-half, half, 0.0),
        Vec3::new(half, half, 0.0),
        Vec3::new(half, -half, 0.0),
        Vec3::new(-half, -half, 0.0),
    ];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &[Vec3::Z; 4],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            indices: Some(Indices::U32(&[0, 2, 1, 0, 3, 2])),
            ..MeshData::default()
        })
        .expect("a quad")
}

pub(crate) fn texture(
    renderer: &mut Renderer,
    width: u32,
    height: u32,
    texel: impl Fn(u32, u32) -> [u8; 4],
) -> TextureBinding {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            data.extend(texel(x, y));
        }
    }
    let id = renderer
        .textures()
        .create(Pixels::rgba8(width, height, data))
        .expect("a texture");
    TextureBinding::new(id, Filtering::NearestClamp)
}

pub(crate) fn looked(frame: &mut Frame) {
    let mut look = rikka_test_renderer_3d::minimal_look();
    let sky = rikka_test_renderer_3d::sky_of(&mut look);
    sky.sun = 0.0;
    sky.moon = 0.0;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    frame.look(look);
}

pub(crate) fn encoded(linear: f32) -> i32 {
    let c = linear.clamp(0.0, 1.0);
    let srgb = match c <= 0.003_130_8 {
        true => c * 12.92,
        false => 1.055 * c.powf(1.0 / 2.4) - 0.055,
    };
    (srgb * 255.0).round() as i32
}

pub(crate) fn near(found: [i32; 3], expected: [i32; 3], tolerance: i32) -> bool {
    found
        .iter()
        .zip(expected)
        .all(|(found, expected)| (found - expected).abs() <= tolerance)
}

pub(crate) const SIZE: u32 = 64;

pub(crate) fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-shader-compile-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

pub(crate) fn compiler() -> ShaderCompiler {
    ShaderCompiler::new().expect("libslang loads")
}

pub(crate) fn compiled(graph: &Graph, library: &Library, compiler: &ShaderCompiler) -> Material {
    Material::compile(graph, library, compiler, "graph").unwrap_or_else(|error| {
        let source = graph
            .material(library, "graph")
            .map(|source| source.source)
            .unwrap_or_default();
        panic!("{error}\n{source}")
    })
}

pub(crate) fn frame() -> Frame {
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(0.2))
        .tone_mapping(ToneMapping::Clamp)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

pub(crate) fn pixels(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport");
    renderer
        .three()
        .render(&mut viewport, frame)
        .expect("a 3D frame draws");
    let mut target = renderer.create_target(SIZE, SIZE).expect("a target");
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

pub(crate) fn rgb(pixels: &[u8], x: u32, y: u32) -> [i32; 3] {
    let at = ((y * SIZE + x) * 4) as usize;
    [0, 1, 2].map(|channel| i32::from(pixels[at + channel]))
}

pub(crate) fn material(renderer: &mut Renderer, desc: MaterialDesc) -> MaterialId {
    renderer.three().create_material(desc).expect("a material")
}

pub(crate) fn lit(mesh: MeshId, materials: &[MaterialId], light: Vec3) -> Frame {
    let mut frame = frame();
    frame.light(
        Light::directional(light)
            .intensity(2.0)
            .shadow(ShadowQuality::None),
    );
    for material in materials {
        frame.draw(Draw::new(mesh, *material));
    }
    frame
}

pub(crate) fn sphere(renderer: &mut Renderer) -> MeshId {
    rikka_test_renderer_3d::sphere(renderer, 1.2, 24).expect("a sphere")
}

pub(crate) fn sphere_of(renderer: &mut Renderer, materials: &[MaterialId], light: Vec3) -> Vec<u8> {
    let mesh = sphere(renderer);
    pixels(renderer, &lit(mesh, materials, light))
}

pub(crate) fn built(
    renderer: &mut Renderer,
    frame: &Frame,
    template: &MaterialTemplate,
) -> Vec<u8> {
    rikka_test_renderer_3d::built(template, || pixels(renderer, frame))
}
