use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Draw, Figure, Frame, Head, Indices, Light, Look, MaterialId, MaterialState,
    MeshData, MeshId, Outline, Overrides, SEE_THROUGH_LAYER, ShadowQuality, ToneMapping, Viewport,
    ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

use crate::support::quad;

const SIZE: u32 = 128;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-look-shadows-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn traced_renderer() -> TestRenderer {
    let renderer = rikka_test_renderer::ray_renderer("rikka-look-shadows-ray-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests");
    assert!(
        renderer.gpu().traces_rays(),
        "these tests need a GPU with acceleration structures and ray queries"
    );
    renderer
}

fn luminance(pixels: &[u8], x: usize, y: usize) -> f32 {
    let i = (y * SIZE as usize + x) * 4;
    0.2126 * f32::from(pixels[i])
        + 0.7152 * f32::from(pixels[i + 1])
        + 0.0722 * f32::from(pixels[i + 2])
}

fn plane(renderer: &mut Renderer, half: f32) -> MeshId {
    let positions = [
        Vec3::new(-half, 0.0, -half),
        Vec3::new(half, 0.0, -half),
        Vec3::new(half, 0.0, half),
        Vec3::new(-half, 0.0, half),
    ];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &[Vec3::Y; 4],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: &[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            indices: Some(Indices::U32(&[0, 2, 1, 0, 3, 2])),
            ..MeshData::default()
        })
        .expect("a plane")
}

fn pbr(renderer: &mut Renderer) -> MaterialId {
    let desc =
        rikka_test_renderer_3d::standard(Vec4::new(0.7, 0.7, 0.7, 1.0), MaterialState::opaque())
            .with("specular_roughness", 0.8)
            .expect("standard parameters");
    renderer
        .three()
        .create_material(desc)
        .expect("a pbr material")
}

fn minimal_look(overrides: Overrides) -> Look {
    let mut look = Look {
        overrides,
        ..rikka_test_renderer_3d::minimal_look()
    };
    rikka_test_renderer_3d::sky_of(&mut look).sun = 0.0;
    rikka_test_renderer_3d::sky_of(&mut look).moon = 0.0;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    look
}

fn frame(camera: Camera, look: Option<Look>) -> Frame {
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Transparent)
        .ambient(Vec3::ZERO)
        .tone_mapping(ToneMapping::Clamp)
        .outline(Outline {
            enabled: false,
            ..Default::default()
        });
    if let Some(look) = look {
        frame.look(look);
    }
    frame
}

fn viewport(renderer: &mut Renderer) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport")
}

fn read(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Vec<u8> {
    for _ in 0..2 {
        renderer
            .three()
            .render(viewport, frame)
            .expect("a 3D frame draws");
    }
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

fn floor_spread(look: Option<Look>) -> f32 {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let floor = plane(&mut renderer, 20.0);
    let material = pbr(&mut renderer);
    let camera = Camera::look_at(
        Vec3::new(0.0, 1.6, 4.0),
        Vec3::new(0.0, 0.0, -2.0),
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    );
    let mut frame = frame(camera, look);
    frame.light(
        Light::directional(Vec3::new(0.3, -0.25, -1.0))
            .intensity(2.0)
            .shadow(ShadowQuality::High),
    );
    frame.draw(Draw::new(floor, material));
    let pixels = read(&mut renderer, &mut view, &frame);
    let mut lowest = f32::MAX;
    let mut highest = f32::MIN;
    for y in (70..124).step_by(3) {
        for x in (8..120).step_by(3) {
            let value = luminance(&pixels, x, y);
            lowest = lowest.min(value);
            highest = highest.max(value);
        }
    }
    highest - lowest
}

#[test]
fn low_sun_floor_has_no_shadow_acne() {
    let plain = floor_spread(None);
    assert!(plain < 24.0, "the floor should be evenly lit: {plain}");
    let looked = floor_spread(Some(minimal_look(Overrides::new())));
    assert!(
        looked < 24.0,
        "the floor should be evenly lit with a look: {looked}"
    );
}

struct Socket {
    eye: MeshId,
    lid: MeshId,
    canopy: MeshId,
    material: MaterialId,
}

impl Socket {
    fn new(renderer: &mut Renderer) -> Self {
        Self {
            eye: quad(renderer, 0.0, 0.02),
            lid: plane(renderer, 0.01),
            canopy: plane(renderer, 1.0),
            material: pbr(renderer),
        }
    }

    fn upper_eye(
        &self,
        renderer: &mut Renderer,
        view: &mut Viewport,
        layers: u32,
        shaded: bool,
    ) -> f32 {
        let camera = Camera::look_at(
            Vec3::new(0.0, 0.0, 0.5),
            Vec3::ZERO,
            Vec3::Y,
            0.12,
            1.0,
            0.05,
        );
        let mut frame = frame(camera, Some(minimal_look(Overrides::new())));
        frame.light(
            Light::directional(Vec3::new(0.0, -1.0, -1.0))
                .intensity(2.0)
                .shadow(ShadowQuality::High),
        );
        let figure = frame.figure(Figure {
            head: Some(Head {
                center: Vec3::new(0.0, 0.0, -0.05),
                forward: Vec3::Z,
                up: Vec3::Y,
                radius: 0.1,
            }),
            ..Figure::default()
        });
        let overhang = Mat4::from_translation(Vec3::new(0.0, 0.022, 0.01))
            * Mat4::from_scale(Vec3::new(3.0, 1.0, 1.0));
        frame
            .draw(
                Draw::new(self.eye, self.material)
                    .figure(figure)
                    .layers(layers),
            )
            .draw(
                Draw::new(self.lid, self.material)
                    .transform(overhang)
                    .figure(figure),
            );
        if shaded {
            frame.draw(
                Draw::new(self.canopy, self.material)
                    .transform(Mat4::from_translation(Vec3::new(0.0, 0.3, 0.0))),
            );
        }
        let pixels = read(renderer, view, &frame);
        luminance(&pixels, 64, 43)
    }
}

#[test]
fn facial_features_take_the_shadow_of_the_face_surface() {
    for (tier, open) in [
        ("raster", renderer as fn() -> TestRenderer),
        ("traced", traced_renderer),
    ] {
        let mut renderer = open();
        let mut view = viewport(&mut renderer);
        let socket = Socket::new(&mut renderer);
        let plain = socket.upper_eye(&mut renderer, &mut view, 0, false);
        let facial = socket.upper_eye(&mut renderer, &mut view, SEE_THROUGH_LAYER, false);
        let covered = socket.upper_eye(&mut renderer, &mut view, SEE_THROUGH_LAYER, true);
        assert!(
            plain < 0.35 * facial,
            "{tier}: the lid shades a plain surface in the socket but not an eye: {plain} of {facial}"
        );
        assert!(
            covered < 0.35 * facial,
            "{tier}: an eye stays shaded by what covers the face: {covered} of {facial}"
        );
    }
}
