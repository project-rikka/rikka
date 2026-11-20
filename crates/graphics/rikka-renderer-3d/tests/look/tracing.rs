use rikka_gpu::Capability;
use rikka_renderer::{Node, OffscreenTarget, Renderer, Scene};
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, Camera, Denoise, Draw, Environment, Frame, Indices, Light, Look,
    MaterialDesc, MaterialId, MaterialState, MaterialTemplate, MeshData, MeshId,
    NITS_PER_RADIANCE_UNIT, Outline, Resampling, ShaderCompiler, ShadowQuality, TemplateReadiness,
    Tier, ToneMapping, Upscale, Viewport, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 128;
const BACKGROUND: f32 = 2.0;

const FIRST_BUILD_CELLS: u32 = 600;

fn traced_renderer() -> TestRenderer {
    let renderer = rikka_test_renderer::ray_renderer("rikka-ray-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests");
    assert!(
        renderer.gpu().traces_rays(),
        "these tests need a GPU with acceleration structures and ray queries"
    );
    renderer
}

fn raster_renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-ray-fallback-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn quad(renderer: &mut Renderer, corners: [Vec3; 4]) -> MeshId {
    let normal = (corners[1] - corners[0])
        .cross(corners[3] - corners[0])
        .normalize();
    let uvs = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &corners,
            normals: &[normal; 4],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: &uvs,
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a quad")
}

fn tessellated(renderer: &mut Renderer, corners: [Vec3; 4], cells: u32) -> MeshId {
    let normal = (corners[1] - corners[0])
        .cross(corners[3] - corners[0])
        .normalize();
    let side = cells + 1;
    let mut positions = Vec::with_capacity((side * side) as usize);
    let mut uvs = Vec::with_capacity(positions.capacity());
    for row in 0..side {
        for column in 0..side {
            let uv = Vec2::new(column as f32, row as f32) / cells as f32;
            let near = corners[0].lerp(corners[1], uv.x);
            let far = corners[3].lerp(corners[2], uv.x);
            positions.push(near.lerp(far, uv.y));
            uvs.push(uv);
        }
    }
    let mut indices = Vec::with_capacity((cells * cells * 6) as usize);
    for row in 0..cells {
        for column in 0..cells {
            let at = row * side + column;
            indices.extend([at, at + 1, at + side + 1, at, at + side + 1, at + side]);
        }
    }
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &vec![normal; positions.len()],
            tangents: &vec![Vec4::new(1.0, 0.0, 0.0, 1.0); positions.len()],
            uvs: &uvs,
            indices: Some(Indices::U32(&indices)),
            ..MeshData::default()
        })
        .expect("a tessellated quad")
}

fn floor(renderer: &mut Renderer, half: f32) -> MeshId {
    quad(
        renderer,
        [
            Vec3::new(-half, 0.0, half),
            Vec3::new(half, 0.0, half),
            Vec3::new(half, 0.0, -half),
            Vec3::new(-half, 0.0, -half),
        ],
    )
}

fn material(renderer: &mut Renderer, albedo: Vec3) -> MaterialId {
    let desc: MaterialDesc = rikka_test_renderer_3d::standard(
        albedo.extend(1.0),
        MaterialState::opaque().double_sided(),
    )
    .with("specular_roughness", 0.9)
    .expect("standard parameters");
    renderer.three().create_material(desc).expect("a material")
}

fn quiet() -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = Environment::None;
    look.shadows.screen.strength = 0.0;
    look
}

fn framed(camera: Camera, look: Look, draws: &[Draw]) -> Frame {
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Color(Vec3::ZERO))
        .tone_mapping(ToneMapping::Clamp)
        .ambient(Vec3::splat(0.03))
        .time(1.0)
        .look(look)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame.draws(draws.iter().cloned());
    frame
}

struct Shots {
    viewport: Viewport,
    target: OffscreenTarget,
}

impl Shots {
    fn new(renderer: &mut Renderer) -> Self {
        Self::antialiased(renderer, Antialiasing::Multisample)
    }

    fn antialiased(renderer: &mut Renderer, antialiasing: Antialiasing) -> Self {
        Self {
            viewport: renderer
                .three()
                .create_viewport(
                    ViewportConfig::new(SIZE, SIZE)
                        .samples(1)
                        .antialiasing(antialiasing),
                )
                .expect("a viewport"),
            target: renderer
                .create_target(SIZE, SIZE)
                .expect("an offscreen target"),
        }
    }

    fn render(&mut self, renderer: &mut Renderer, frame: &Frame) -> rikka_renderer_3d::Result<()> {
        renderer.three().render(&mut self.viewport, frame)
    }

    fn read(&mut self, renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
        self.render(renderer, frame).expect("a 3D frame draws");
        let mut scene = Scene::new();
        scene.set_background(Color::rgb(0.0, 0.0, 0.0));
        scene.insert(Node::image(
            Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
            self.viewport.texture(),
        ));
        renderer
            .draw(&mut scene, &mut self.target)
            .expect("a 2D frame draws");
        renderer
            .read_pixels(&mut self.target)
            .expect("pixels come back")
    }

    fn settle(&mut self, renderer: &mut Renderer, frame: &Frame, frames: usize) -> Vec<u8> {
        for _ in 1..frames {
            self.render(renderer, frame).expect("a 3D frame draws");
        }
        self.read(renderer, frame)
    }

    fn built(
        &mut self,
        renderer: &mut Renderer,
        frame: &Frame,
        template: &MaterialTemplate,
    ) -> Vec<u8> {
        rikka_test_renderer_3d::built(template, || self.read(renderer, frame))
    }
}

fn rgb(pixels: &[u8], (x, y): (usize, usize)) -> Vec3 {
    let mut sum = Vec3::ZERO;
    for dy in 0..3 {
        for dx in 0..3 {
            let i = ((y + dy - 1) * SIZE as usize + x + dx - 1) * 4;
            sum += Vec3::new(
                f32::from(pixels[i]),
                f32::from(pixels[i + 1]),
                f32::from(pixels[i + 2]),
            );
        }
    }
    sum / 9.0
}

fn brightness(pixels: &[u8], at: usize) -> f32 {
    let i = at * 4;
    (f32::from(pixels[i]) + f32::from(pixels[i + 1]) + f32::from(pixels[i + 2])) / 3.0
}

fn linear_brightness(pixels: &[u8], at: usize) -> f32 {
    let decode = |byte: u8| {
        let encoded = f32::from(byte) / 255.0;
        match encoded <= 0.04045 {
            true => encoded / 12.92,
            false => ((encoded + 0.055) / 1.055).powf(2.4),
        }
    };
    let i = at * 4;
    (decode(pixels[i]) + decode(pixels[i + 1]) + decode(pixels[i + 2])) * 255.0 / 3.0
}

fn pixel_of(camera: &Camera, world: Vec3) -> (usize, usize) {
    let clip = camera.view_projection() * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    (
        ((ndc.x * 0.5 + 0.5) * SIZE as f32) as usize,
        ((ndc.y * 0.5 + 0.5) * SIZE as f32) as usize,
    )
}

fn shadowed(pixels: &[u8]) -> Vec<bool> {
    let levels: Vec<f32> = (0..(SIZE * SIZE) as usize)
        .map(|at| brightness(pixels, at))
        .collect();
    let lit = levels.iter().copied().fold(0.0, f32::max);
    levels
        .iter()
        .map(|&level| level > BACKGROUND && level < lit * 0.5)
        .collect()
}

fn mismatch(a: &[bool], b: &[bool]) -> f32 {
    let differing = a.iter().zip(b).filter(|(a, b)| a != b).count();
    differing as f32 / a.len() as f32
}

fn centroid(mask: &[bool]) -> Vec2 {
    let mut sum = Vec2::ZERO;
    let mut count: f32 = 0.0;
    for (at, _) in mask.iter().enumerate().filter(|&(_, &dark)| dark) {
        sum += Vec2::new((at % SIZE as usize) as f32, (at / SIZE as usize) as f32);
        count += 1.0;
    }
    sum / count.max(1.0)
}

fn overhead() -> Camera {
    Camera::look_at(
        Vec3::new(-1.0, 5.0, 4.0),
        Vec3::new(0.6, 0.0, 0.0),
        Vec3::Y,
        0.9,
        1.0,
        0.05,
    )
}

fn sun() -> Light {
    Light::directional(Vec3::new(1.0, -1.0, 0.3))
        .intensity(3.0)
        .shadow(ShadowQuality::High)
}

fn shadow_look(tier: Tier) -> Look {
    let mut look = quiet();
    look.shadows.tier = tier;
    look
}

#[test]
fn ray_traced_shadow_covers_what_the_shadow_map_covers() {
    let mut renderer = traced_renderer();
    let ground = floor(&mut renderer, 3.0);
    let blocker = tessellated(
        &mut renderer,
        [
            Vec3::new(-0.6, 1.0, 0.6),
            Vec3::new(0.6, 1.0, 0.6),
            Vec3::new(0.6, 1.0, -0.6),
            Vec3::new(-0.6, 1.0, -0.6),
        ],
        FIRST_BUILD_CELLS,
    );
    let grey = material(&mut renderer, Vec3::splat(0.7));
    let draws = [Draw::new(ground, grey), Draw::new(blocker, grey)];
    let camera = overhead();
    let mut shots = Shots::new(&mut renderer);
    let frames = |tier| {
        let mut frame = framed(camera, shadow_look(tier), &draws);
        frame.light(sun());
        frame
    };
    let mapped = shadowed(&shots.settle(&mut renderer, &frames(Tier::ScreenSpace), 4));
    let traced = shadowed(&shots.settle(&mut renderer, &frames(Tier::RayTraced), 12));
    let covered = traced.iter().filter(|&&dark| dark).count() as f32 / traced.len() as f32;
    assert!(covered > 0.015, "the blocker casts a shadow: {covered}");
    let differing = mismatch(&mapped, &traced);
    assert!(
        differing < 0.03,
        "{differing} of the pixels disagree between the shadow map and the traced shadow"
    );
}

#[test]
fn ray_traced_shadow_falls_on_blended_surfaces_beyond_the_shadow_maps() {
    let mut renderer = traced_renderer();
    let receiver = floor(&mut renderer, 3.0);
    let blocker = quad(
        &mut renderer,
        [
            Vec3::new(-0.6, 1.0, 0.6),
            Vec3::new(0.6, 1.0, 0.6),
            Vec3::new(0.6, 1.0, -0.6),
            Vec3::new(-0.6, 1.0, -0.6),
        ],
    );
    let grey = material(&mut renderer, Vec3::splat(0.7));
    let glass = renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::new(0.7, 0.7, 0.7, 0.8),
            MaterialState::transparent().double_sided(),
        ))
        .expect("a blended material");
    let draws = [Draw::new(receiver, glass), Draw::new(blocker, grey)];
    let camera = overhead();
    let under = pixel_of(&camera, Vec3::new(1.0, 0.0, 0.3));
    let open = pixel_of(&camera, Vec3::new(-1.6, 0.0, -1.2));
    let mut shots = Shots::new(&mut renderer);
    let mut contrast = |tier| {
        let mut look = shadow_look(tier);
        look.shadows.distance = 0.3;
        let mut frame = framed(camera, look, &draws);
        frame.light(sun());
        let pixels = shots.settle(&mut renderer, &frame, 12);
        let lit = rgb(&pixels, open).length();
        assert!(lit > 20.0, "the blended receiver is lit: {lit}");
        rgb(&pixels, under).length() / lit
    };
    let mapped = contrast(Tier::ScreenSpace);
    assert!(
        mapped > 0.8,
        "the shadow maps end before the receiver: {mapped}"
    );
    let traced = contrast(Tier::RayTraced);
    assert!(
        traced < 0.5,
        "the blocker shadows the blended receiver: {traced}"
    );
}

const SHIFTED: &str = r#"
import rikka;

struct Surface : ISurface<RikkaStandardData>, IVertex {
    [Default(0.0)] float shift;
    [Role("displacement")] [Default(2.0)] float reach;

    [constref]
    RikkaSurfaceData<RikkaStandardData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaStandardData data;
        data.metallic = 0.0;
        data.roughness = 1.0;
        RikkaSurfaceData<RikkaStandardData> s = rikka_surface_data(pixel, data);
        s.albedo = float3(0.7);
        return s;
    }

    [constref]
    void deform(inout RikkaVertex vertex, RikkaContext context, float time) {
        vertex.world_position.x += shift;
    }
};

typealias Model = RikkaStandardModel;
"#;

#[test]
fn ray_traced_shadow_follows_vertex_hooks() {
    let mut renderer = traced_renderer();
    let ground = floor(&mut renderer, 3.0);
    let blocker = quad(
        &mut renderer,
        [
            Vec3::new(-0.5, 1.0, 0.5),
            Vec3::new(0.5, 1.0, 0.5),
            Vec3::new(0.5, 1.0, -0.5),
            Vec3::new(-0.5, 1.0, -0.5),
        ],
    );
    let grey = material(&mut renderer, Vec3::splat(0.7));
    let template = ShaderCompiler::new()
        .expect("libslang loads")
        .template("shifted", SHIFTED)
        .expect("a vertex hook compiles");
    let shifted = renderer
        .three()
        .create_material(
            MaterialDesc::new(&template)
                .state(MaterialState::opaque().double_sided())
                .with("shift", -1.0)
                .expect("the template declares shift"),
        )
        .expect("a shifted material");
    let camera = overhead();
    let mut shots = Shots::new(&mut renderer);
    let frames = |tier| {
        let mut frame = framed(camera, shadow_look(tier), &[Draw::new(ground, grey)]);
        frame.light(sun());
        frame.draw(Draw::new(blocker, shifted));
        frame
    };
    let rest = shadowed(&shots.settle(&mut renderer, &frames(Tier::RayTraced), 4));
    renderer
        .three()
        .set_material(shifted, "shift", 1.0)
        .expect("shift is a parameter");
    let moved = shadowed(&shots.settle(&mut renderer, &frames(Tier::RayTraced), 12));
    let mapped = shadowed(&shots.settle(&mut renderer, &frames(Tier::ScreenSpace), 4));
    let shift = centroid(&moved).x - centroid(&rest).x;
    let expected = pixel_of(&camera, Vec3::new(1.0, 0.0, 0.0)).0 as f32
        - pixel_of(&camera, Vec3::new(-1.0, 0.0, 0.0)).0 as f32;
    assert!(
        (shift - expected).abs() < expected.abs() * 0.15,
        "the traced shadow moved {shift} px, the hook {expected} px"
    );
    let differing = mismatch(&mapped, &moved);
    assert!(
        differing < 0.03,
        "{differing} of the pixels disagree with the shadow map of the hooked caster"
    );
}

#[test]
fn ray_traced_view_optimizes_its_traced_variants_when_it_appears() {
    let mut renderer = traced_renderer();
    let ground = floor(&mut renderer, 3.0);
    let blocker = quad(
        &mut renderer,
        [
            Vec3::new(-0.5, 1.0, 0.5),
            Vec3::new(0.5, 1.0, 0.5),
            Vec3::new(0.5, 1.0, -0.5),
            Vec3::new(-0.5, 1.0, -0.5),
        ],
    );
    let template = ShaderCompiler::new()
        .expect("libslang loads")
        .template("shifted", SHIFTED)
        .expect("a vertex hook compiles");
    let shifted = renderer
        .three()
        .create_material(
            MaterialDesc::new(&template)
                .state(MaterialState::opaque().double_sided())
                .with("shift", 0.0)
                .expect("the template declares shift"),
        )
        .expect("a shifted material");
    let camera = overhead();
    let mut shots = Shots::new(&mut renderer);
    let frames = |tier| {
        let mut frame = framed(camera, shadow_look(tier), &[Draw::new(ground, shifted)]);
        frame.light(sun());
        frame.draw(Draw::new(blocker, shifted));
        frame
    };
    let mapped = shadowed(&shots.built(&mut renderer, &frames(Tier::ScreenSpace), &template));
    shots.read(&mut renderer, &frames(Tier::RayTraced));
    assert!(
        matches!(template.readiness(), TemplateReadiness::Drawable),
        "the traced variants wait for their optimized build"
    );
    let traced = shadowed(&shots.built(&mut renderer, &frames(Tier::RayTraced), &template));
    let covered = traced.iter().filter(|&&dark| dark).count() as f32 / traced.len() as f32;
    assert!(
        covered > 0.015,
        "the blocker casts a traced shadow: {covered}"
    );
    let differing = mismatch(&mapped, &traced);
    assert!(
        differing < 0.03,
        "{differing} of the pixels disagree between the shadow map and the traced shadow"
    );
}

fn skinned_blocker(renderer: &mut Renderer) -> MeshId {
    let corners = [
        Vec3::new(-0.5, 1.0, 0.5),
        Vec3::new(0.5, 1.0, 0.5),
        Vec3::new(0.5, 1.0, -0.5),
        Vec3::new(-0.5, 1.0, -0.5),
    ];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &corners,
            normals: &[Vec3::Y; 4],
            joints: &[[0, 0, 0, 0]; 4],
            weights: &[Vec4::X; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a skinned quad")
}

#[test]
fn ray_traced_shadow_follows_skinned_motion() {
    let mut renderer = traced_renderer();
    let ground = floor(&mut renderer, 3.0);
    let blocker = skinned_blocker(&mut renderer);
    let grey = material(&mut renderer, Vec3::splat(0.7));
    let camera = overhead();
    let mut shots = Shots::new(&mut renderer);
    let posed = |tier, offset: f32| {
        let mut frame = framed(camera, shadow_look(tier), &[Draw::new(ground, grey)]);
        frame.light(sun());
        let skin = frame.skin(&[Mat4::from_translation(Vec3::new(offset, 0.0, 0.0))]);
        frame.draw(Draw::new(blocker, grey).skin(skin));
        frame
    };
    let rest = shadowed(&shots.settle(&mut renderer, &posed(Tier::RayTraced, -1.0), 4));
    let moved = shadowed(&shots.settle(&mut renderer, &posed(Tier::RayTraced, 1.0), 12));
    let mapped = shadowed(&shots.settle(&mut renderer, &posed(Tier::ScreenSpace, 1.0), 4));
    let shift = centroid(&moved).x - centroid(&rest).x;
    let expected = pixel_of(&camera, Vec3::new(1.0, 0.0, 0.0)).0 as f32
        - pixel_of(&camera, Vec3::new(-1.0, 0.0, 0.0)).0 as f32;
    assert!(
        (shift - expected).abs() < expected.abs() * 0.15,
        "the traced shadow moved {shift} px, the bone {expected} px"
    );
    let differing = mismatch(&mapped, &moved);
    assert!(
        differing < 0.03,
        "{differing} of the pixels disagree with the shadow map after the bone moved"
    );
}

struct Cornell {
    draws: Vec<Draw>,
    camera: Camera,
}

fn cornell(renderer: &mut Renderer) -> Cornell {
    let red = material(renderer, Vec3::new(0.75, 0.04, 0.04));
    cornell_with(renderer, red)
}

fn cornell_with(renderer: &mut Renderer, red: MaterialId) -> Cornell {
    let white = material(renderer, Vec3::splat(0.75));
    let green = material(renderer, Vec3::new(0.04, 0.75, 0.04));
    let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let walls = [
        (
            [
                p(-1., 0., 1.),
                p(1., 0., 1.),
                p(1., 0., -1.),
                p(-1., 0., -1.),
            ],
            white,
        ),
        (
            [
                p(-1., 2., -1.),
                p(1., 2., -1.),
                p(1., 2., 1.),
                p(-1., 2., 1.),
            ],
            white,
        ),
        (
            [
                p(-1., 0., -1.),
                p(1., 0., -1.),
                p(1., 2., -1.),
                p(-1., 2., -1.),
            ],
            white,
        ),
        (
            [
                p(-1., 0., 1.),
                p(-1., 0., -1.),
                p(-1., 2., -1.),
                p(-1., 2., 1.),
            ],
            red,
        ),
        (
            [p(1., 0., -1.), p(1., 0., 1.), p(1., 2., 1.), p(1., 2., -1.)],
            green,
        ),
    ];
    let draws = walls
        .into_iter()
        .map(|(corners, paint)| Draw::new(quad(renderer, corners), paint))
        .collect();
    Cornell {
        draws,
        camera: Camera::look_at(
            Vec3::new(0.0, 1.0, 3.4),
            Vec3::new(0.0, 0.9, 0.0),
            Vec3::Y,
            0.8,
            1.0,
            0.05,
        ),
    }
}

fn lit_box(scene: &Cornell, look: Look) -> Frame {
    let mut frame = framed(scene.camera, look, &scene.draws);
    frame.ambient(Vec3::ZERO);
    frame.light(Light::point(Vec3::new(0.0, 1.85, 0.0), 6.0).intensity(6.0));
    frame
}

fn bounce_look(tier: Tier) -> Look {
    let mut look = quiet();
    look.illumination.tier = tier;
    look.illumination.strength = 1.0;
    look
}

#[test]
fn illumination_bleeds_the_red_wall_into_the_corner() {
    for tier in [Tier::RayTraced, Tier::ScreenSpace] {
        let mut renderer = traced_renderer();
        let scene = cornell(&mut renderer);
        let mut shots = Shots::new(&mut renderer);
        let corner = pixel_of(&scene.camera, Vec3::new(-0.85, 0.0, 0.0));
        let plain = rgb(
            &shots.settle(&mut renderer, &lit_box(&scene, bounce_look(Tier::Off)), 2),
            corner,
        );
        let bounced = rgb(
            &shots.settle(&mut renderer, &lit_box(&scene, bounce_look(tier)), 24),
            corner,
        );
        let gained = bounced - plain;
        assert!(
            gained.x > 4.0 && gained.x > gained.y + 2.0 && gained.x > gained.z + 2.0,
            "{tier:?}: the floor beside the red wall went from {plain} to {bounced}"
        );
    }
}

fn occlusion_look(tier: Tier) -> Look {
    let mut look = quiet();
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    let sky = rikka_test_renderer_3d::sky_of(&mut look);
    sky.ground = Vec3::new(0.9, 0.04, 0.02);
    let sun = sky.to_sun();
    sky.heading = std::f32::consts::PI - sun.x.atan2(sun.z);
    look.occlusion.tier = tier;
    look.occlusion.strength = 1.0;
    look
}

#[test]
fn traced_and_screen_space_occlusion_agree_on_a_crease() {
    let mut renderer = traced_renderer();
    let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let wall = quad(
        &mut renderer,
        [
            p(-3., 0., -0.5),
            p(3., 0., -0.5),
            p(3., 1.5, -0.5),
            p(-3., 1.5, -0.5),
        ],
    );
    let roof = quad(
        &mut renderer,
        [
            p(-3., 1.5, -0.5),
            p(3., 1.5, -0.5),
            p(3., 1.5, 0.9),
            p(-3., 1.5, 0.9),
        ],
    );
    let grey = material(&mut renderer, Vec3::splat(0.7));
    let draws = [Draw::new(wall, grey), Draw::new(roof, grey)];
    let camera = Camera::look_at(p(0.0, 0.8, 2.6), p(0.0, 1.0, -0.5), Vec3::Y, 1.0, 1.0, 0.05);
    let mut shot = |tier: Tier, frames: usize| {
        let mut shots = Shots::new(&mut renderer);
        shots.settle(
            &mut renderer,
            &framed(camera, occlusion_look(tier), &draws),
            frames,
        )
    };
    let open = shot(Tier::Off, 2);
    let screen = shot(Tier::ScreenSpace, 2);
    let traced = shot(Tier::RayTraced, 32);
    for point in [p(0.0, 1.42, -0.49), p(0.0, 1.3, -0.49), p(0.0, 1.49, -0.35)] {
        let at = pixel_of(&camera, point);
        let (open, screen, traced) = (rgb(&open, at), rgb(&screen, at), rgb(&traced, at));
        assert!(
            screen.z < open.z * 0.95 && traced.z < open.z * 0.95,
            "occlusion hides the sky above {point}: {open}, screen space {screen}, ray traced {traced}"
        );
        let apart = (traced - screen).abs().max_element();
        assert!(
            apart <= 0.1 * open.max_element(),
            "at {point}, unoccluded {open}: ray traced {traced} and screen space {screen}"
        );
    }
}

#[test]
fn traced_bounces_light_hits_only_through_shared_channels() {
    let mut renderer = traced_renderer();
    let scene = cornell(&mut renderer);
    let above = Camera::look_at(
        Vec3::new(-0.5, 1.8, 0.0),
        Vec3::new(-0.5, 0.0, 0.0),
        Vec3::NEG_Z,
        0.3,
        1.0,
        0.05,
    );
    let mut shots = Shots::new(&mut renderer);
    let corner = pixel_of(&above, Vec3::new(-0.72, 0.0, 0.0));
    let mut bleed = |red_channels: u32| {
        let draws: Vec<Draw> = scene
            .draws
            .iter()
            .enumerate()
            .map(|(index, draw)| draw.light_channels(if index == 3 { red_channels } else { 3 }))
            .collect();
        let frame = |tier: Tier| {
            let mut frame = framed(above, bounce_look(tier), &draws);
            frame.ambient(Vec3::ZERO);
            frame.light(
                Light::point(Vec3::new(0.0, 1.85, 0.0), 6.0)
                    .intensity(6.0)
                    .channels(2),
            );
            frame
        };
        let plain = rgb(&shots.settle(&mut renderer, &frame(Tier::Off), 2), corner);
        let bounced = rgb(
            &shots.settle(&mut renderer, &frame(Tier::RayTraced), 24),
            corner,
        );
        let gained = bounced - plain;
        gained.x - gained.y.max(gained.z)
    };
    let shared = bleed(3);
    let excluded = bleed(1);
    assert!(
        shared > 2.0 && excluded < 0.6 * shared,
        "a hit outside the light's channels reflects only light that reached it indirectly: \
         red excess {shared} vs {excluded}"
    );
}

const TURNED: &str = r#"
import rikka;

struct Surface : ISurface<RikkaStandardData>, IVertex {
    [Color] [Default(0.75, 0.04, 0.04)] [Role("albedo")] float3 color;
    [Role("displacement")] [Default(0.0)] float reach;

    [constref]
    RikkaSurfaceData<RikkaStandardData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaStandardData data;
        data.metallic = 0.0;
        data.roughness = 1.0;
        RikkaSurfaceData<RikkaStandardData> s = rikka_surface_data(pixel, data);
        s.albedo = color;
        return s;
    }

    [constref]
    void deform(inout RikkaVertex vertex, RikkaContext context, float time) {
        vertex.normal = float3(1.0, 0.0, 0.0);
    }
};

typealias Model = RikkaStandardModel;
"#;

#[test]
fn traced_bounce_shades_hooked_walls_by_their_deformed_normals() {
    let mut renderer = traced_renderer();
    let mut scene = cornell(&mut renderer);
    let corners = [
        Vec3::new(-1.0, 0.0, 1.0),
        Vec3::new(-1.0, 0.0, -1.0),
        Vec3::new(-1.0, 2.0, -1.0),
        Vec3::new(-1.0, 2.0, 1.0),
    ];
    let downward = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &corners,
            normals: &[Vec3::NEG_Y; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a wall facing down");
    let template = ShaderCompiler::new()
        .expect("libslang loads")
        .template("turned", TURNED)
        .expect("a normal hook compiles");
    let turned = renderer
        .three()
        .create_material(MaterialDesc::new(&template).state(MaterialState::opaque().double_sided()))
        .expect("a turned material");
    scene.draws[3] = Draw::new(downward, turned);
    let mut shots = Shots::new(&mut renderer);
    let corner = pixel_of(&scene.camera, Vec3::new(-0.85, 0.0, 0.0));
    let plain = rgb(
        &shots.settle(&mut renderer, &lit_box(&scene, bounce_look(Tier::Off)), 2),
        corner,
    );
    let bounced = rgb(
        &shots.settle(
            &mut renderer,
            &lit_box(&scene, bounce_look(Tier::RayTraced)),
            24,
        ),
        corner,
    );
    let gained = bounced - plain;
    assert!(
        gained.x > 4.0 && gained.x > gained.y + 2.0 && gained.x > gained.z + 2.0,
        "the floor beside the turned red wall went from {plain} to {bounced}"
    );
}

#[test]
fn traced_bounces_read_openpbr_roles() {
    let mut renderer = traced_renderer();
    let opaque = material(&mut renderer, Vec3::new(0.75, 0.04, 0.04));
    let paint = |desc: MaterialDesc| {
        desc.state(MaterialState::opaque().double_sided())
            .with("specular_roughness", 0.9)
            .expect("standard parameters")
    };
    let glass =
        rikka_test_renderer_3d::standard(Vec4::new(0.75, 0.04, 0.04, 1.0), MaterialState::opaque())
            .with("transmission_weight", 1.0)
            .expect("a transmission weight");
    let glowing =
        rikka_test_renderer_3d::standard(Vec4::new(0.0, 0.0, 0.0, 1.0), MaterialState::opaque())
            .with("emission_color", Vec3::new(1.0, 0.05, 0.05))
            .and_then(|desc| desc.with("emission_luminance", 0.6 * NITS_PER_RADIANCE_UNIT))
            .expect("an emission");
    let glass = renderer
        .three()
        .create_material(paint(glass))
        .expect("a material");
    let glowing = renderer
        .three()
        .create_material(paint(glowing))
        .expect("a material");
    let mut gain = |wall: MaterialId, lit: bool| {
        let scene = cornell_with(&mut renderer, wall);
        let mut shots = Shots::new(&mut renderer);
        let corner = pixel_of(&scene.camera, Vec3::new(-0.85, 0.0, 0.0));
        let frame = |tier| match lit {
            true => lit_box(&scene, bounce_look(tier)),
            false => {
                let mut frame = framed(scene.camera, bounce_look(tier), &scene.draws);
                frame.ambient(Vec3::ZERO);
                frame
            }
        };
        let plain = rgb(&shots.settle(&mut renderer, &frame(Tier::Off), 2), corner);
        let bounced = rgb(
            &shots.settle(&mut renderer, &frame(Tier::RayTraced), 24),
            corner,
        );
        bounced - plain
    };
    let reflected = gain(opaque, true);
    let transmitted = gain(glass, true);
    let emitted = gain(glowing, false);
    assert!(
        transmitted.x < 0.5 * reflected.x,
        "a transmissive wall bounces less red than an opaque one: {transmitted} against {reflected}"
    );
    assert!(
        emitted.x > 4.0 && emitted.x > emitted.y + 2.0,
        "an emissive wall lights the corner through traced bounces: {emitted}"
    );
}

#[test]
fn denoised_illumination_settles() {
    let mut renderer = traced_renderer();
    let scene = cornell(&mut renderer);
    let mut shots = Shots::new(&mut renderer);
    let frame = lit_box(&scene, bounce_look(Tier::RayTraced));
    let mut previous = shots.read(&mut renderer, &frame);
    let mut changes = Vec::new();
    for _ in 0..24 {
        let next = shots.read(&mut renderer, &frame);
        let change = previous
            .iter()
            .zip(&next)
            .map(|(&a, &b)| (f32::from(a) - f32::from(b)).abs())
            .sum::<f32>()
            / next.len() as f32;
        changes.push(change);
        previous = next;
    }
    let early = changes[..3].iter().sum::<f32>() / 3.0;
    let late = changes[changes.len() - 4..].iter().sum::<f32>() / 4.0;
    assert!(
        late < early * 0.5 && late < 0.5,
        "frame-to-frame change went from {early} to {late}"
    );
}

fn every_tier(tier: Tier) -> Look {
    let mut look = quiet();
    look.shadows.tier = tier;
    look.occlusion.tier = tier;
    look.occlusion.strength = 1.0;
    look.reflections.tier = tier;
    look.reflections.strength = 1.0;
    look.illumination.tier = tier;
    look.illumination.strength = 1.0;
    look
}

#[test]
fn auto_tiers_fall_back_to_screen_space_without_ray_tracing() {
    let mut renderer = raster_renderer();
    let scene = cornell(&mut renderer);
    let mut shots = Shots::new(&mut renderer);
    let auto = shots.settle(&mut renderer, &lit_box(&scene, every_tier(Tier::Auto)), 3);
    let mut shots = Shots::new(&mut renderer);
    let screen = shots.settle(
        &mut renderer,
        &lit_box(&scene, every_tier(Tier::ScreenSpace)),
        3,
    );
    assert!(auto == screen, "Auto draws what ScreenSpace draws");
}

#[test]
fn ray_traced_tiers_fail_without_ray_tracing() {
    let mut renderer = raster_renderer();
    let scene = cornell(&mut renderer);
    let mut shots = Shots::new(&mut renderer);
    type Demand = fn(&mut Look);
    let demands: [(&str, Demand); 4] = [
        ("shadows", |look| look.shadows.tier = Tier::RayTraced),
        ("occlusion", |look| look.occlusion.tier = Tier::RayTraced),
        ("reflections", |look| {
            look.reflections.tier = Tier::RayTraced
        }),
        ("illumination", |look| {
            look.illumination.tier = Tier::RayTraced
        }),
    ];
    for (effect, demand) in demands {
        let mut look = every_tier(Tier::ScreenSpace);
        demand(&mut look);
        let refused = shots.render(&mut renderer, &lit_box(&scene, look));
        assert!(
            matches!(
                refused,
                Err(Error::Gpu(rikka_renderer::Error::NotEnabled(
                    Capability::RayTracing
                )))
            ),
            "ray traced {effect} on a raster device gave {refused:?}"
        );
    }
}

fn change(a: &[u8], b: &[u8]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(&a, &b)| (f32::from(a) - f32::from(b)).abs())
        .sum::<f32>()
        / a.len() as f32
}

fn traced_box(renderer: &mut Renderer, scene: &Cornell) -> Frame {
    let blocker = quad(
        renderer,
        [
            Vec3::new(-0.3, 1.2, 0.3),
            Vec3::new(0.3, 1.2, 0.3),
            Vec3::new(0.3, 1.2, -0.3),
            Vec3::new(-0.3, 1.2, -0.3),
        ],
    );
    let grey = material(renderer, Vec3::splat(0.6));
    let mut look = bounce_look(Tier::RayTraced);
    look.shadows.tier = Tier::RayTraced;
    let mut frame = framed(scene.camera, look, &scene.draws);
    frame
        .ambient(Vec3::ZERO)
        .light(
            Light::point(Vec3::new(0.0, 1.85, 0.0), 6.0)
                .source_size(0.05)
                .intensity(6.0)
                .shadow(ShadowQuality::High),
        )
        .draw(Draw::new(blocker, grey));
    frame
}

#[test]
fn traced_lighting_settles_under_temporal_antialiasing() {
    const FRAMES: usize = 300;
    const ACCUMULATING: std::ops::Range<usize> = 40..60;
    let mut renderer = traced_renderer();
    let scene = cornell(&mut renderer);
    let frame = traced_box(&mut renderer, &scene);
    let mut reference = Shots::new(&mut renderer);
    let reference = reference.settle(&mut renderer, &frame, 48);
    let corner = pixel_of(&scene.camera, Vec3::new(-0.85, 0.0, 0.0));
    for antialiasing in [
        Antialiasing::Temporal,
        Antialiasing::Upscale(Upscale::fixed(0.67)),
        Antialiasing::Upscale(Upscale::fixed(0.5)),
    ] {
        let mut shots = Shots::antialiased(&mut renderer, antialiasing);
        let mut previous = shots.read(&mut renderer, &frame);
        let mut changes = Vec::with_capacity(FRAMES);
        for _ in 1..FRAMES {
            let next = shots.read(&mut renderer, &frame);
            changes.push(change(&previous, &next));
            previous = next;
        }
        let accumulating = changes[ACCUMULATING].iter().copied().fold(0.0, f32::max);
        let settled = changes[changes.len() - 8..]
            .iter()
            .copied()
            .fold(0.0, f32::max);
        let distance = change(&previous, &reference);
        let bled = rgb(&previous, corner);
        assert!(
            accumulating < 0.3,
            "{antialiasing:?}: frames still change by {accumulating} while accumulating"
        );
        assert_eq!(
            settled, 0.0,
            "{antialiasing:?}: settled frames keep changing"
        );
        assert!(
            distance < 3.0,
            "{antialiasing:?}: the settled image is {distance} away from the native traced one"
        );
        assert!(
            bled.x > bled.y + 2.0 && bled.x > bled.z + 2.0,
            "{antialiasing:?}: the red wall no longer bleeds into the corner: {bled}"
        );
    }
}

#[test]
fn camera_cut_drops_traced_histories() {
    let mut renderer = traced_renderer();
    let scene = cornell(&mut renderer);
    let lit = traced_box(&mut renderer, &scene);
    let mut look = bounce_look(Tier::RayTraced);
    look.shadows.tier = Tier::RayTraced;
    let mut dark = framed(scene.camera, look, &scene.draws);
    dark.ambient(Vec3::ZERO).time(2.0);
    for antialiasing in [Antialiasing::Multisample, Antialiasing::Temporal] {
        let mut shots = Shots::antialiased(&mut renderer, antialiasing);
        shots.settle(&mut renderer, &lit, 32);
        dark.cut();
        let after = shots.read(&mut renderer, &dark);
        let brightest = (0..(SIZE * SIZE) as usize)
            .map(|at| brightness(&after, at))
            .fold(0.0, f32::max);
        assert!(
            brightest < BACKGROUND,
            "{antialiasing:?}: the unlit box still glows at {brightest} after a cut"
        );
    }
}

#[test]
fn ray_traced_shadows_respect_light_channels() {
    let mut renderer = traced_renderer();
    let ground = floor(&mut renderer, 3.0);
    let blocker = quad(
        &mut renderer,
        [
            Vec3::new(-0.6, 1.0, 0.6),
            Vec3::new(0.6, 1.0, 0.6),
            Vec3::new(0.6, 1.0, -0.6),
            Vec3::new(-0.6, 1.0, -0.6),
        ],
    );
    let grey = material(&mut renderer, Vec3::splat(0.7));
    let camera = overhead();
    let mut shots = Shots::new(&mut renderer);
    let under = pixel_of(&camera, Vec3::new(1.0, 0.0, 0.3));
    let open = pixel_of(&camera, Vec3::new(-1.5, 0.0, 1.5));
    let mut shade = |channels: u32| {
        let draws = [
            Draw::new(ground, grey).light_channels(2),
            Draw::new(blocker, grey).light_channels(channels),
        ];
        let mut frame = framed(camera, shadow_look(Tier::RayTraced), &draws);
        frame.light(sun().channels(2));
        let pixels = shots.settle(&mut renderer, &frame, 8);
        let level = |at: (usize, usize)| {
            let lit = rgb(&pixels, at);
            (lit.x + lit.y + lit.z) / 3.0
        };
        level(under) / level(open).max(1.0)
    };
    let unlinked = shade(1);
    let linked = shade(3);
    assert!(
        unlinked > 0.9 && linked < 0.5,
        "only a caster sharing the light's channel shadows it: {unlinked} vs {linked}"
    );
}

fn glowing(renderer: &mut Renderer, luminance: f32) -> MaterialId {
    let desc = rikka_test_renderer_3d::standard(
        Vec4::new(0.0, 0.0, 0.0, 1.0),
        MaterialState::opaque().double_sided(),
    )
    .with("emission_color", Vec3::ONE)
    .and_then(|desc| desc.with("emission_luminance", luminance * NITS_PER_RADIANCE_UNIT))
    .and_then(|desc| desc.with("specular_roughness", 0.9))
    .expect("an emission");
    renderer
        .three()
        .create_material(desc)
        .expect("a glowing material")
}

fn resampled_look(resampling: Resampling) -> Look {
    let mut look = bounce_look(Tier::RayTraced);
    look.illumination.resampling = resampling;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    look
}

fn raw_bounces(resampling: Resampling) -> Look {
    let mut look = resampled_look(resampling);
    look.denoise = Denoise {
        history: 1.0,
        passes: 0,
    };
    look
}

#[test]
fn resampling_lowers_the_noise_of_raw_traced_bounces() {
    const WARMING: usize = 16;
    const MEASURED: usize = 16;
    const REFERENCE: usize = 128;
    let mut renderer = traced_renderer();
    let mut scene = cornell(&mut renderer);
    let patch = quad(
        &mut renderer,
        [
            Vec3::new(-0.4, 1.98, -0.4),
            Vec3::new(0.4, 1.98, -0.4),
            Vec3::new(0.4, 1.98, 0.4),
            Vec3::new(-0.4, 1.98, 0.4),
        ],
    );
    let glow = glowing(&mut renderer, 2.0);
    scene.draws.push(Draw::new(patch, glow));
    let frame = |resampling| {
        let mut frame = framed(scene.camera, raw_bounces(resampling), &scene.draws);
        frame.ambient(Vec3::ZERO);
        frame
    };
    let ceiling = pixel_of(&scene.camera, Vec3::new(0.0, 1.98, 0.0)).1;
    let size = SIZE as usize;
    let rows = match ceiling < size / 2 {
        true => size / 2 + 8..size - 8,
        false => 8..size / 2 - 8,
    };
    let region: Vec<usize> = rows
        .flat_map(|y| (16..size - 16).map(move |x| y * size + x))
        .collect();
    let measure = |shots: &mut Shots, renderer: &mut Renderer, frame: &Frame| {
        let mut frames = Vec::with_capacity(MEASURED);
        shots.settle(renderer, frame, WARMING);
        for _ in 0..MEASURED {
            let pixels = shots.read(renderer, frame);
            frames.push(
                region
                    .iter()
                    .map(|&at| linear_brightness(&pixels, at))
                    .collect::<Vec<f32>>(),
            );
        }
        frames
    };
    let plain_frame = frame(Resampling::Off);
    let mut plain = Shots::new(&mut renderer);
    let plain_frames = measure(&mut plain, &mut renderer, &plain_frame);
    let mut reference = vec![0.0; region.len()];
    for _ in 0..REFERENCE {
        let pixels = plain.read(&mut renderer, &plain_frame);
        for (sum, &at) in reference.iter_mut().zip(&region) {
            *sum += linear_brightness(&pixels, at) / REFERENCE as f32;
        }
    }
    let error = |frames: &[Vec<f32>]| {
        frames
            .iter()
            .flat_map(|levels| {
                levels
                    .iter()
                    .zip(&reference)
                    .map(|(a, b)| (a - b) * (a - b))
            })
            .sum::<f32>()
            / (frames.len() * reference.len()) as f32
    };
    let mut resampled = Shots::new(&mut renderer);
    let resampled_frames = measure(
        &mut resampled,
        &mut renderer,
        &frame(Resampling::Spatiotemporal),
    );
    let mean = reference.iter().sum::<f32>() / reference.len() as f32;
    let resampled_mean = resampled_frames.iter().flatten().sum::<f32>()
        / (resampled_frames.len() * reference.len()) as f32;
    let plain_error = error(&plain_frames);
    let resampled_error = error(&resampled_frames);
    assert!(
        mean > 4.0 && resampled_error < 0.15 * plain_error,
        "the room lit by a glowing patch (mean level {mean}) has a squared error of \
         {resampled_error} with resampling against {plain_error} without"
    );
    assert!(
        (resampled_mean - mean).abs() < 0.25 * mean,
        "resampling keeps the bounced energy: {resampled_mean} against {mean}"
    );
}

#[test]
fn resampled_bounces_do_not_leak_through_a_thin_wall() {
    let mut renderer = traced_renderer();
    let p = Vec3::new;
    let ground = floor(&mut renderer, 1.5);
    let wall = quad(
        &mut renderer,
        [
            p(0., 0., -1.5),
            p(0., 0., 1.5),
            p(0., 1., 1.5),
            p(0., 1., -1.5),
        ],
    );
    let lamp = quad(
        &mut renderer,
        [
            p(-1., 0., -1.),
            p(-1., 0., 1.),
            p(-1., 0.8, 1.),
            p(-1., 0.8, -1.),
        ],
    );
    let grey = material(&mut renderer, Vec3::splat(0.75));
    let glow = glowing(&mut renderer, 2.0);
    let draws = [
        Draw::new(ground, grey),
        Draw::new(wall, grey),
        Draw::new(lamp, glow),
    ];
    let camera = Camera::look_at(p(0.0, 4.0, 0.0), Vec3::ZERO, Vec3::NEG_Z, 0.9, 1.0, 0.05);
    let side = |pixels: &[u8], from: f32, to: f32| {
        let mut sum = 0.0;
        let mut count = 0.0;
        for step in 0..8 {
            let x = from + (to - from) * step as f32 / 7.0;
            for row in 0..8 {
                let z = -0.6 + 1.2 * row as f32 / 7.0;
                let (column, line) = pixel_of(&camera, p(x, 0.0, z));
                sum += brightness(pixels, line * SIZE as usize + column);
                count += 1.0;
            }
        }
        sum / count
    };
    let mut shade = |resampling| {
        let mut frame = framed(camera, raw_bounces(resampling), &draws);
        frame.ambient(Vec3::ZERO);
        let mut shots = Shots::new(&mut renderer);
        let pixels = shots.settle(&mut renderer, &frame, 16);
        (side(&pixels, -0.5, -0.15), side(&pixels, 0.15, 0.5))
    };
    let (plain_lit, plain_dark) = shade(Resampling::Off);
    let (lit, dark) = shade(Resampling::Spatiotemporal);
    assert!(
        plain_lit > 20.0 && lit > 20.0,
        "the glowing panel lights its side of the wall: {plain_lit} and {lit}"
    );
    assert!(
        dark < plain_dark + 1.0,
        "the far side of the wall stays dark with resampling: {dark} against {plain_dark}"
    );
}

#[test]
fn switching_the_light_off_clears_resampled_bounces_within_frames() {
    const FRAMES: usize = 16;
    const CLEARED: f32 = 0.1;
    let mut renderer = traced_renderer();
    let scene = cornell(&mut renderer);
    let every = 0..(SIZE * SIZE) as usize;
    let brightest = |pixels: &[u8]| {
        every
            .clone()
            .map(|at| brightness(pixels, at))
            .fold(0.0, f32::max)
    };
    let mean = |pixels: &[u8]| {
        every
            .clone()
            .map(|at| linear_brightness(pixels, at))
            .sum::<f32>()
            / every.len() as f32
    };
    let switched = |renderer: &mut Renderer, look: Look| {
        let lit = lit_box(&scene, look.clone());
        let mut dark = framed(scene.camera, look, &scene.draws);
        dark.ambient(Vec3::ZERO);
        let mut shots = Shots::new(renderer);
        let before = shots.settle(renderer, &lit, 32);
        let after: Vec<Vec<u8>> = (0..FRAMES).map(|_| shots.read(renderer, &dark)).collect();
        (before, after)
    };
    let (_, raw) = switched(&mut renderer, raw_bounces(Resampling::Spatiotemporal));
    let levels: Vec<f32> = raw.iter().map(|pixels| brightest(pixels)).collect();
    assert!(
        levels.iter().any(|&level| level < BACKGROUND),
        "raw resampled bounces still glow {FRAMES} frames after the light went off: {levels:?}"
    );
    let (lit, denoised) = switched(&mut renderer, resampled_look(Resampling::Spatiotemporal));
    let lit = mean(&lit);
    let levels: Vec<f32> = denoised.iter().map(|pixels| mean(pixels) / lit).collect();
    assert!(
        levels.iter().any(|&level| level < CLEARED),
        "denoised resampled bounces keep more than {CLEARED} of the lit box {FRAMES} frames \
         after the light went off: {levels:?}"
    );
}

#[test]
fn resampled_bounces_repeat_exactly_for_identical_frames() {
    let mut renderer = traced_renderer();
    let scene = cornell(&mut renderer);
    let frame = lit_box(&scene, bounce_look(Tier::RayTraced));
    let run = |renderer: &mut Renderer| {
        let mut shots = Shots::new(renderer);
        (0..12)
            .map(|_| shots.read(renderer, &frame))
            .collect::<Vec<_>>()
    };
    let first = run(&mut renderer);
    let second = run(&mut renderer);
    assert!(
        first == second,
        "two viewports drawing the same frames trace the same reservoirs"
    );
}
