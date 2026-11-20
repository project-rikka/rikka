use rikka_gpu::{Filtering, Pixels};
use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Frame, Indices, Light, Look, MaterialDesc, MaterialId, MaterialState,
    MaterialTemplate, MeshData, MeshId, Outline, ShadowQuality, ToneMapping, Value, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

pub(crate) const SIZE: u32 = 128;

pub(crate) fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-shader-presets-gpu-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

pub(crate) fn preset(key: &str) -> MaterialTemplate {
    rikka_shader_presets_gpu::template(key).expect("the preset is prebuilt")
}

pub(crate) fn opaque(key: &str) -> MaterialDesc {
    MaterialDesc::new(&preset(key)).state(MaterialState::opaque())
}

pub(crate) fn create(renderer: &mut Renderer, desc: MaterialDesc) -> MaterialId {
    renderer.three().create_material(desc).expect("a material")
}

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
) -> Value {
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
    Some(rikka_renderer_3d::TextureBinding::new(
        id,
        Filtering::NearestClamp,
    ))
    .into()
}

pub(crate) fn looked() -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    let sky = rikka_test_renderer_3d::sky_of(&mut look);
    sky.sun = 0.0;
    sky.moon = 0.0;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    look
}

pub(crate) fn frame(look: Option<Look>) -> Frame {
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
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    if let Some(look) = look {
        frame.look(look);
    }
    frame
}

pub(crate) fn sun(towards: Vec3) -> Light {
    Light::directional(-towards)
        .intensity(3.0)
        .shadow(ShadowQuality::None)
}

pub(crate) struct Shot(Vec<u8>);

impl Shot {
    pub(crate) fn take(renderer: &mut Renderer, frame: &Frame) -> Shot {
        let mut view = renderer
            .three()
            .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
            .expect("a viewport");
        for _ in 0..2 {
            renderer
                .three()
                .render(&mut view, frame)
                .expect("a 3D frame draws");
        }
        let mut target = renderer
            .create_target(SIZE, SIZE)
            .expect("an offscreen target");
        let mut scene = Scene::new();
        scene.set_background(Color::rgb(0.0, 0.0, 0.0));
        scene.insert(Node::image(
            Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
            view.texture(),
        ));
        renderer
            .draw(&mut scene, &mut target)
            .expect("a 2D frame draws");
        Shot(renderer.read_pixels(&mut target).expect("pixels come back"))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn at(&self, x: u32, y: u32) -> Vec3 {
        let i = ((y * SIZE + x) * 4) as usize;
        Vec3::new(
            f32::from(self.0[i]),
            f32::from(self.0[i + 1]),
            f32::from(self.0[i + 2]),
        )
    }

    pub(crate) fn brightness(&self, x: std::ops::Range<u32>, y: u32) -> f32 {
        let count = x.len() as f32;
        x.map(|x| self.at(x, y).element_sum()).sum::<f32>() / count
    }
}

pub(crate) fn encoded(linear: Vec3) -> Vec3 {
    let channel = |value: f32| {
        let c = value.clamp(0.0, 1.0);
        let srgb = match c <= 0.003_130_8 {
            true => c * 12.92,
            false => 1.055 * c.powf(1.0 / 2.4) - 0.055,
        };
        (srgb * 255.0).round()
    };
    Vec3::new(channel(linear.x), channel(linear.y), channel(linear.z))
}
