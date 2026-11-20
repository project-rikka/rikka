use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Frame, Indices, MaterialId, MaterialState, MeshData, MeshId, Viewport,
    ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

pub(crate) const SIZE: u32 = 128;

pub(crate) fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-mesh-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

pub(crate) fn quad(renderer: &mut Renderer, z: f32, half: f32) -> MeshId {
    let positions = [
        Vec3::new(-half, -half, z),
        Vec3::new(half, -half, z),
        Vec3::new(half, half, z),
        Vec3::new(-half, half, z),
    ];
    let normals = [Vec3::Z; 4];
    let tangents = [Vec4::new(1.0, 0.0, 0.0, 1.0); 4];
    let uvs = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];
    let indices = [0u32, 1, 2, 0, 2, 3];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            tangents: &tangents,
            uvs: &uvs,
            indices: Some(Indices::U32(&indices)),
            ..MeshData::default()
        })
        .expect("a quad is a mesh")
}

pub(crate) fn flat(renderer: &mut Renderer, color: [f32; 4]) -> MaterialId {
    renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::from_array(color),
            MaterialState::opaque(),
        ))
        .expect("a material")
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

pub(crate) fn ambient_frame(ambient: f32) -> Frame {
    let mut frame = Frame::new(camera());
    frame
        .background(Background::Transparent)
        .ambient(Vec3::splat(ambient))
        .outline(rikka_renderer_3d::Outline {
            enabled: false,
            ..Default::default()
        });
    frame
}

pub(crate) fn read(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Vec<u8> {
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

pub(crate) fn viewport(renderer: &mut Renderer, samples: u32) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(samples))
        .expect("a viewport")
}
