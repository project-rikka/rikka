use rikka_gpu::Compute;
use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Draw, Frame, Indices, Light, MaterialId, MaterialState, MeshData, MeshId,
    Outline, ShadowQuality, ToneMapping, ViewportConfig, Wetness,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 64;
const CORNERS: usize = 4;

fn renderer() -> TestRenderer {
    rikka_test_renderer::fluid_renderer("rikka-wetness-test")
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
        Self {
            mesh: card(renderer),
            material: rough(renderer),
        }
    }

    fn draw(self) -> Draw {
        Draw::new(self.mesh, self.material)
    }
}

fn card(renderer: &mut Renderer) -> MeshId {
    let half = 0.8;
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &[
                Vec3::new(-half, -half, 0.0),
                Vec3::new(half, -half, 0.0),
                Vec3::new(half, half, 0.0),
                Vec3::new(-half, half, 0.0),
            ],
            normals: &[Vec3::Z; CORNERS],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); CORNERS],
            uvs: &[
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a card")
}

fn rough(renderer: &mut Renderer) -> MaterialId {
    renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::new(0.8, 0.8, 0.8, 1.0),
            MaterialState::opaque(),
        ))
        .expect("a rough material")
}

fn frame(looked: bool) -> Frame {
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame
        .background(Background::Color(Vec3::ZERO))
        .tone_mapping(ToneMapping::Clamp)
        .ambient(Vec3::splat(0.2))
        .light(
            Light::directional(Vec3::new(0.5, -0.3, -1.0))
                .intensity(1.0)
                .shadow(ShadowQuality::None),
        )
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    if looked {
        let mut look = rikka_test_renderer_3d::minimal_look();
        rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
        frame.look(look);
    }
    frame
}

fn deposit(
    compute: &Compute,
    frame: &mut Frame,
    tint: Vec3,
    amount: f32,
    vertices: usize,
) -> Wetness {
    let bytes: Vec<u8> = std::iter::repeat_n((tint * amount).extend(amount), vertices)
        .flat_map(|vertex| vertex.to_array())
        .flat_map(f32::to_le_bytes)
        .collect();
    let buffer = compute
        .buffer(bytes.len() as u64)
        .expect("a wetness buffer");
    let mut batch = compute.batch();
    batch.write(&buffer, 0, &bytes);
    let ticket = batch.submit().expect("the wetness goes up");
    frame.wetness(&buffer, &ticket)
}

fn render(renderer: &mut Renderer, frame: &Frame) -> Result<[f32; 3], Error> {
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
    let pixels = renderer.read_pixels(&mut target)?;
    let middle = ((SIZE / 2) * SIZE + SIZE / 2) as usize * 4;
    Ok([0, 1, 2].map(|channel| linear(pixels[middle + channel])))
}

fn linear(encoded: u8) -> f32 {
    let encoded = f32::from(encoded) / 255.0;
    match encoded <= 0.04045 {
        true => encoded / 12.92,
        false => ((encoded + 0.055) / 1.055).powf(2.4),
    }
}

fn soaked(
    renderer: &mut Renderer,
    compute: &Compute,
    card: Card,
    looked: bool,
    tint: Vec3,
    amount: f32,
) -> [f32; 3] {
    let mut frame = frame(looked);
    let wetness = deposit(compute, &mut frame, tint, amount, CORNERS);
    frame.draw(card.draw().wet(wetness));
    render(renderer, &frame).expect("a wet card draws")
}

#[test]
fn wetness_darkens_rough_surface_only() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let card = Card::new(&mut renderer);
    for looked in [false, true] {
        let dry = soaked(&mut renderer, &compute, card, looked, Vec3::ONE, 0.0);
        let wet = soaked(&mut renderer, &compute, card, looked, Vec3::ONE, 1.0);
        let mut plain = frame(looked);
        plain.draw(card.draw());
        let plain = render(&mut renderer, &plain).expect("a dry card draws");

        assert_eq!(dry, plain, "looked {looked}");
        assert!(
            wet[1] < 0.88 * dry[1],
            "looked {looked}: a soaked card went from {dry:?} to {wet:?}"
        );
    }
}

#[test]
fn rejects_wetness_past_buffer_end() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let card = Card::new(&mut renderer);
    let mut short = frame(false);
    let wetness = deposit(&compute, &mut short, Vec3::ONE, 1.0, CORNERS - 1);
    short.draw(card.draw().wet(wetness));
    assert!(matches!(
        render(&mut renderer, &short),
        Err(Error::Invalid(_))
    ));

    let mut shifted = frame(false);
    let wetness = deposit(&compute, &mut shifted, Vec3::ONE, 1.0, CORNERS);
    shifted.draw(card.draw().wet(wetness.with_first_vertex(1)));
    assert!(matches!(
        render(&mut renderer, &shifted),
        Err(Error::Invalid(_))
    ));
}
