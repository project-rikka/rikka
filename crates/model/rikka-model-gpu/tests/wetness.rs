use std::sync::Arc;

use rikka_model_core::glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{Material, Mesh, Model, ModelBuilder, Node, NodeId, Submesh, Vertices};
use rikka_model_gpu::GpuModel;
use rikka_renderer::{Node as SceneNode, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{Background, Camera, Frame, Outline, ViewportConfig};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 64;
const LEFT: f32 = -0.6;
const RIGHT: f32 = 0.6;

fn renderer() -> TestRenderer {
    rikka_test_renderer::fluid_renderer("rikka-model-wetness-test")
        .expect("a Vulkan 1.3 device with mesh and compute support is required for these tests")
}

fn card(centre: f32) -> Mesh {
    let half = 0.5;
    Mesh {
        vertices: Vertices {
            positions: vec![
                Vec3::new(centre - half, -half, 0.0),
                Vec3::new(centre + half, -half, 0.0),
                Vec3::new(centre + half, half, 0.0),
                Vec3::new(centre - half, half, 0.0),
            ],
            normals: vec![Vec3::Z; 4],
            tangents: vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Mesh::new("card")
    }
}

fn two_cards() -> Arc<Model> {
    let mut builder = ModelBuilder::new().importer("test");
    let grey =
        builder.add_material(Material::new("grey").base_color(Vec4::new(0.8, 0.8, 0.8, 1.0)));
    let meshes: Vec<_> = [LEFT, RIGHT]
        .into_iter()
        .map(|centre| {
            let mut mesh = card(centre);
            mesh.submeshes = vec![Submesh {
                index_count: 6,
                material: Some(grey),
                ..Submesh::default()
            }];
            builder.add_mesh(mesh)
        })
        .collect();
    builder.add_node(Node::new("right").mesh(meshes[1]));
    builder.add_node(Node::new("left").mesh(meshes[0]));
    Arc::new(builder.finish().expect("two cards are a model"))
}

fn pixels(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport");
    renderer
        .three()
        .render(&mut viewport, frame)
        .expect("a 3D frame draws");
    let mut target = renderer
        .create_target(SIZE, SIZE)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(SceneNode::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    renderer.read_pixels(&mut target).expect("pixels come back")
}

fn redness(pixels: &[u8], camera: &Camera, x: f32) -> f32 {
    let clip = camera.view_projection() * Vec3::new(x, 0.0, 0.0).extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let column = ((ndc.x * 0.5 + 0.5) * SIZE as f32) as usize;
    let row = ((ndc.y * 0.5 + 0.5) * SIZE as f32) as usize;
    let at = (row * SIZE as usize + column) * 4;
    f32::from(pixels[at]) / f32::from(pixels[at + 1]).max(1.0)
}

#[test]
fn wetness_offsets_follow_node_order() {
    let mut renderer = renderer();
    let compute = renderer.gpu().compute().expect("compute work");
    let gpu = GpuModel::upload(&mut renderer, two_cards()).expect("an uploaded model");
    let instance = gpu.instance();
    let right = gpu
        .wet_offset(NodeId(0))
        .expect("the right card's first vertex") as usize;
    let left = gpu
        .wet_offset(NodeId(1))
        .expect("the left card's first vertex") as usize;
    assert_eq!(
        (right, left),
        (0, 4),
        "nodes lie in node order, as a body does"
    );
    assert_eq!(gpu.wet_vertices(), 8);

    let soaked = (0..gpu.wet_vertices() as usize)
        .map(|vertex| match vertex < left {
            true => Vec4::new(1.0, 0.1, 0.1, 1.0),
            false => Vec4::ZERO,
        })
        .flat_map(|vertex| vertex.to_array())
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<u8>>();
    let buffer = compute
        .buffer(soaked.len() as u64)
        .expect("a wetness buffer");
    let mut batch = compute.batch();
    batch.write(&buffer, 0, &soaked);
    let ticket = batch.submit().expect("the wetness goes up");

    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    );
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Transparent)
        .ambient(Vec3::ONE)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    let wetness = frame.wetness(&buffer, &ticket);
    gpu.drawing(&instance).wet(wetness).draw_into(&mut frame);
    let pixels = pixels(&mut renderer, &frame);

    let dry = redness(&pixels, &camera, LEFT);
    let wet = redness(&pixels, &camera, RIGHT);
    assert!(dry < 1.1, "the dry card is {dry} times redder than green");
    assert!(
        wet > 1.8,
        "the red-soaked card is only {wet} times redder than green"
    );
}
