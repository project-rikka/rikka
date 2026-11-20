use std::f32::consts::FRAC_PI_2;

use rikka_renderer::{Node, Renderer, Scene};

use rikka_gpu::Compute;
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, DRAPE_VERTEX_BYTES, Drape, Draw, Frame, Indices, Light, MaterialId,
    MaterialState, MeshData, MeshId, Outline, ShadowQuality, ToneMapping, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 64;
const HALF: f32 = 0.4;
const LOCAL: [Vec3; 4] = [
    Vec3::new(-HALF, -HALF, 0.0),
    Vec3::new(HALF, -HALF, 0.0),
    Vec3::new(HALF, HALF, 0.0),
    Vec3::new(-HALF, HALF, 0.0),
];

fn renderer() -> TestRenderer {
    rikka_test_renderer::fluid_renderer("rikka-drape-test")
        .expect("a Vulkan 1.3 device with mesh and compute support is required for these tests")
}

fn compute(renderer: &TestRenderer) -> Compute {
    renderer
        .gpu()
        .compute()
        .expect("the test device can run compute work")
}

#[derive(Clone, Copy)]
struct Card {
    mesh: MeshId,
    material: MaterialId,
}

impl Card {
    fn new(renderer: &mut Renderer) -> Self {
        let mesh = renderer
            .three()
            .create_mesh(&MeshData {
                positions: &LOCAL,
                normals: &[Vec3::Z; 4],
                tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
                joints: &[[0, 0, 0, 0]; 4],
                weights: &[Vec4::X; 4],
                indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
                ..MeshData::default()
            })
            .expect("a skinned card");
        let material = renderer
            .three()
            .create_material(rikka_test_renderer_3d::standard(
                Vec4::new(1.0, 0.0, 0.0, 1.0),
                MaterialState::opaque(),
            ))
            .expect("a red material");
        Self { mesh, material }
    }

    fn bone() -> Mat4 {
        Mat4::from_translation(Vec3::new(-0.15, 0.05, 0.0))
    }

    fn transform() -> Mat4 {
        Mat4::from_translation(Vec3::new(0.1, 0.0, 0.0)) * Mat4::from_rotation_z(FRAC_PI_2)
    }

    fn skinned_world(vertex: usize) -> Vec3 {
        (Self::transform() * Self::bone()).transform_point3(LOCAL[vertex])
    }

    fn draw(self, frame: &mut Frame) -> Draw {
        let skin = frame.skin(&[Self::bone()]);
        Draw::new(self.mesh, self.material)
            .transform(Self::transform())
            .skin(skin)
    }
}

struct Draped {
    position: Vec3,
    blend: f32,
    normal: Vec3,
}

fn upload(compute: &Compute, frame: &mut Frame, vertices: &[Draped]) -> Drape {
    let floats: Vec<f32> = vertices
        .iter()
        .flat_map(|vertex| {
            [
                vertex.position.x,
                vertex.position.y,
                vertex.position.z,
                vertex.blend,
                vertex.normal.x,
                vertex.normal.y,
                vertex.normal.z,
                0.0,
                0.0,
                1.0,
                0.0,
                1.0,
            ]
        })
        .collect();
    let bytes: Vec<u8> = floats.into_iter().flat_map(f32::to_le_bytes).collect();
    assert_eq!(
        bytes.len() as u64,
        vertices.len() as u64 * DRAPE_VERTEX_BYTES
    );
    let buffer = compute.buffer(bytes.len() as u64).expect("a drape buffer");
    let mut batch = compute.batch();
    batch.write(&buffer, 0, &bytes);
    let ticket = batch.submit().expect("the drape goes up");
    frame.drape(&buffer, &ticket)
}

fn frame(light: Vec3) -> Frame {
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame
        .background(Background::Transparent)
        .ambient(Vec3::ZERO)
        .tone_mapping(ToneMapping::Clamp)
        .light(
            Light::directional(light)
                .intensity(3.0)
                .shadow(ShadowQuality::None),
        )
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

fn pixels(renderer: &mut Renderer, frame: &Frame) -> Result<Vec<u8>, Error> {
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))?;
    renderer.three().render(&mut viewport, frame)?;
    let mut target = renderer.create_target(SIZE, SIZE)?;
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer.draw(&mut scene, &mut target)?;
    Ok(renderer.read_pixels(&mut target)?)
}

fn lit_columns(pixels: &[u8]) -> Vec<usize> {
    let row = SIZE as usize / 2;
    (0..SIZE as usize)
        .filter(|&x| pixels[(row * SIZE as usize + x) * 4] > 40)
        .collect()
}

#[test]
fn drape_blends_only_weighted_vertices() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let card = Card::new(&mut renderer);

    let mut skinned = frame(Vec3::NEG_Z);
    let draw = card.draw(&mut skinned);
    skinned.draw(draw);
    let resting = lit_columns(&pixels(&mut renderer, &skinned).expect("a skinned card draws"));

    let middle = (0..4).map(Card::skinned_world).sum::<Vec3>().x / 4.0;
    let vertices: Vec<Draped> = (0..4)
        .map(|vertex| {
            let world = Card::skinned_world(vertex);
            match world.x > middle {
                true => Draped {
                    position: world + Vec3::new(0.6, 0.0, 0.0),
                    blend: 1.0,
                    normal: Vec3::Z,
                },
                false => Draped {
                    position: Vec3::splat(100.0),
                    blend: 0.0,
                    normal: Vec3::NEG_Z,
                },
            }
        })
        .collect();
    let mut stretched = frame(Vec3::NEG_Z);
    let drape = upload(&compute, &mut stretched, &vertices);
    let draw = card.draw(&mut stretched).with_drape(drape);
    stretched.draw(draw);
    let widened = lit_columns(&pixels(&mut renderer, &stretched).expect("a draped card draws"));

    assert!(!resting.is_empty() && !widened.is_empty());
    assert_eq!(widened.first(), resting.first());
    assert!(
        widened.last() > resting.last().map(|last| last + 8).as_ref(),
        "resting {resting:?}, draped {widened:?}"
    );
}

#[test]
fn rejects_drape_shorter_than_mesh() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let card = Card::new(&mut renderer);
    let short: Vec<Draped> = (0..3)
        .map(|vertex| Draped {
            position: Card::skinned_world(vertex),
            blend: 1.0,
            normal: Vec3::Z,
        })
        .collect();

    let mut frame = frame(Vec3::NEG_Z);
    let drape = upload(&compute, &mut frame, &short);
    let draw = card.draw(&mut frame).with_drape(drape);
    frame.draw(draw);
    assert!(matches!(
        pixels(&mut renderer, &frame),
        Err(Error::Invalid(_))
    ));
}
